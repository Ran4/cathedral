use super::*;
use crate::checkpoint::{CheckpointBudget, Cohort};
use serde_json::json;

fn restored(t: &WeatherTimeline) -> Admitted<WeatherCandidate> {
    let b = CheckpointBudget::default();
    let saved = t
        .export_checkpoint(b.reserve(Cohort::SavePayload, 4096).unwrap())
        .unwrap()
        .encode()
        .unwrap();
    let candidate = WeatherTimelineDtoV1::decode(
        saved.value(),
        b.reserve(Cohort::LoadCandidate, saved.value().len() + 4096)
            .unwrap(),
    )
    .unwrap()
    .into_candidate()
    .unwrap();
    assert_eq!(&candidate.value().timeline, t);
    candidate
}
#[test]
fn checkpoint_scheduled_climate_and_lightning_continue_with_stable_crossing_ids() {
    let timeline = WeatherTimeline::new(WeatherConfig::default()).with_climate(WeatherClimate {
        precipitation_chance_per_slot: 1.0,
        fog_chance_per_day: 0.8,
        drizzle_share: 0.0,
        rain_share: 0.0,
        downpour_share: 0.0,
        thunderstorm_share: 1.0,
        minimum_wet_hours: 6.0,
        maximum_wet_hours: 8.0,
    });
    let candidate = restored(&timeline);
    let mut kinds = std::collections::BTreeSet::new();
    let mut strikes = std::collections::BTreeSet::new();
    for quarter in -96..=384 {
        let at = quarter as f64 / 96.0;
        let sample = timeline.sample(at);
        kinds.insert(sample.kind);
        assert_eq!(candidate.value().sample(at).unwrap(), sample);
        let events = timeline.lightning_crossed(at - 1.0 / 96.0, at);
        assert_eq!(
            candidate
                .value()
                .timeline
                .lightning_crossed(at - 1.0 / 96.0, at),
            events
        );
        for strike in events {
            assert!(strikes.insert(strike.id));
        }
    }
    assert!(kinds.len() >= 4 && kinds.contains(&WeatherKind::Thunderstorm));
    assert!(!strikes.is_empty());
}
#[test]
fn checkpoint_override_inherits_water_and_clear_keeps_drying_residue() {
    let mut control = WeatherTimeline::new(WeatherConfig::default());
    control.set_override(WeatherKind::Downpour, Some(0.9), 2.0);
    let wet = control.sample(2.2);
    assert!(wet.surface_wetness > 0.9 && wet.standing_water > 0.8);
    control.set_override(WeatherKind::Clear, None, 2.2);
    assert!(control.forced.unwrap().initial_wetness > 0.9);
    let saved = restored(&control);
    assert_eq!(saved.value().sample(2.23).unwrap(), control.sample(2.23));
    // These independently retained controls are test state, not a full-envelope
    // coexistence claim. Admission remains with the actual decoded candidate.
    let mut continuation = saved.value().timeline.clone();
    control.clear_override(2.23);
    continuation.clear_override(2.23);
    assert!(control.residue.unwrap().wetness > 0.0);
    assert_eq!(control, continuation);
    let residue = restored(&continuation);
    for at in [2.23, 2.24, 2.4, 3.0, 10.0] {
        assert_eq!(residue.value().sample(at).unwrap(), control.sample(at));
    }
}
#[test]
fn checkpoint_unanchored_and_wrapping_overrides_preserve_their_actual_origin() {
    for config in [
        WeatherConfig::default(),
        WeatherConfig {
            enabled: false,
            frequency: 0.0,
            ..Default::default()
        },
        WeatherConfig {
            mode: WeatherMode::Forced(WeatherKind::Rain),
            ..Default::default()
        },
    ] {
        let mut t = WeatherTimeline::new(config);
        restored(&t);
        t.set_override(WeatherKind::Thunderstorm, None, f64::NAN);
        assert_eq!(t.forced.unwrap().began_at_days, None);
        let c = restored(&t);
        assert_eq!(c.value().sample(-1.0).unwrap(), t.sample(-1.0));
        t.next_override_revision = u64::MAX;
        t.set_override(WeatherKind::Rain, Some(0.4), -0.25);
        assert_eq!(t.next_override_revision, u64::MAX / 2);
        restored(&t);
        t.clear_override(0.1);
        restored(&t);
        t.clear_override(f64::NAN);
        restored(&t);
    }
}
#[test]
fn checkpoint_weather_strict_corruption_releases_admission() {
    let mut t = WeatherTimeline::new(WeatherConfig::default());
    t.set_override(WeatherKind::Rain, Some(0.8), 2.0);
    let b = CheckpointBudget::default();
    let saved = t
        .export_checkpoint(b.reserve(Cohort::SavePayload, 4096).unwrap())
        .unwrap()
        .encode()
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(saved.value()).unwrap();
    let retained = b.retained_bytes();
    for mutate in [
        |v: &mut serde_json::Value| {
            v["timeline"].as_object_mut().unwrap().remove("forced");
        },
        |v: &mut serde_json::Value| {
            v["timeline"].as_object_mut().unwrap().remove("residue");
        },
        |v: &mut serde_json::Value| {
            v["timeline"]["forced"]
                .as_object_mut()
                .unwrap()
                .remove("intensity");
        },
        |v: &mut serde_json::Value| {
            v["timeline"]["forced"]
                .as_object_mut()
                .unwrap()
                .remove("began_at_days");
        },
        |v: &mut serde_json::Value| {
            v["timeline"]["forced"]["began_at_days"] = json!(1e30);
        },
        |v: &mut serde_json::Value| {
            v["timeline"]["forced"]["initial_wetness"] = json!(1.01);
        },
        |v: &mut serde_json::Value| {
            v["timeline"]["next_override_revision"] = json!(0);
        },
        |v: &mut serde_json::Value| {
            v["timeline"]["climate"]["rain_share"] = json!(-1);
        },
        |v: &mut serde_json::Value| {
            v["timeline"]["config"]["unknown"] = json!(false);
        },
        |v: &mut serde_json::Value| {
            v["timeline"]["residue"] =
                json!({"cleared_at_days":2.0,"wetness":0.2,"standing_water":0.1});
        },
    ] {
        let mut bad = v.clone();
        mutate(&mut bad);
        let bytes = serde_json::to_vec(&bad).unwrap();
        assert!(
            WeatherTimelineDtoV1::decode(
                &bytes,
                b.reserve(Cohort::LoadCandidate, bytes.len() + 4096)
                    .unwrap()
            )
            .is_err()
        );
        assert_eq!(b.retained_bytes(), retained);
    }
    let duplicate = String::from_utf8(saved.value().clone()).unwrap().replace(
        "\"next_override_revision\":",
        "\"next_override_revision\":0,\"next_override_revision\":",
    );
    let err = WeatherTimelineDtoV1::decode(
        duplicate.as_bytes(),
        b.reserve(Cohort::LoadCandidate, duplicate.len() + 4096)
            .unwrap(),
    )
    .unwrap_err();
    assert!(err.reason.contains("duplicate field"));
}
#[test]
fn checkpoint_weather_layout_and_sampling_scratch_are_closed_and_bounded() {
    for (name, n, limit) in [
        (
            "WeatherTimeline",
            std::mem::size_of::<WeatherTimeline>(),
            256,
        ),
        ("ForcedWeather", std::mem::size_of::<ForcedWeather>(), 80),
        ("WeatherResidue", std::mem::size_of::<WeatherResidue>(), 32),
        ("WeatherSample", std::mem::size_of::<WeatherSample>(), 128),
        ("Episode", std::mem::size_of::<Episode>(), 192),
        ("Shower", std::mem::size_of::<Shower>(), 32),
    ] {
        println!("{name}={n}");
        assert!(n <= limit);
    }
    // 129 knots + 35 days * (2 episodes * 6 boundaries + 2 fog edges)
    // = 619 doubles, capacity <=1024 from the initial 512. Charge sort scratch
    // a full second capacity, plus the independently bounded water buffers.
    assert_eq!(129 + 35 * (2 * 6 + 2), 619);
    let working = 2 * 1024 * 8
        + 16 * std::mem::size_of::<Episode>()
        + 32 * std::mem::size_of::<Shower>()
        + 32 * 16
        + 4096;
    println!("sampling_working_upper_bytes={working}");
    assert!(working < VALIDATION_WORKING_BYTES);
}
fn fixture_timelines() -> [(&'static str, WeatherTimeline); 3] {
    let initial = WeatherTimeline::new(WeatherConfig::default());
    let mut forced = initial.clone();
    forced.set_override(WeatherKind::Downpour, Some(0.9), 2.0);
    forced.set_override(WeatherKind::Clear, None, 2.2);
    let mut residue = forced.clone();
    residue.clear_override(2.23);
    [
        ("weather_initial", initial),
        ("weather_forced", forced),
        ("weather_residue", residue),
    ]
}
fn fixture_bytes(t: &WeatherTimeline) -> Admitted<Vec<u8>> {
    let b = CheckpointBudget::default();
    t.export_checkpoint(b.reserve(Cohort::SavePayload, 4096).unwrap())
        .unwrap()
        .encode()
        .unwrap()
}
#[test]
fn checkpoint_weather_supported_fixtures_preserve_all_private_shapes() {
    let expected = [
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/checkpoint_v1/weather_initial.json"
        ))
        .as_slice(),
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/checkpoint_v1/weather_forced.json"
        ))
        .as_slice(),
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/checkpoint_v1/weather_residue.json"
        ))
        .as_slice(),
    ];
    for ((name, t), expected) in fixture_timelines().into_iter().zip(expected) {
        assert_eq!(fixture_bytes(&t).value().as_slice(), expected, "{name}");
        restored(&t);
    }
}
#[test]
#[ignore = "explicit supported M2a4 weather fixture writer"]
fn write_checkpoint_weather_fixtures() {
    let root =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/checkpoint_v1");
    for (name, t) in fixture_timelines() {
        std::fs::write(root.join(format!("{name}.json")), fixture_bytes(&t).value()).unwrap();
    }
}
