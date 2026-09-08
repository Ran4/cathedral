use super::*;
use crate::{
    NullSight, NullTranscription, NullTts,
    checkpoint::{CheckpointBudget, Cohort},
};
use serde_json::json;

struct Unavailable;
impl Cognition for Unavailable {
    fn request(
        &mut self,
        _: String,
    ) -> std::result::Result<crate::RequestId, crate::CognitionBusy> {
        Err(crate::CognitionBusy)
    }
}
fn engine() -> Engine {
    Engine::new(
        EngineConfig {
            fake_mode: true,
            clock: WorldClock::new(60.0, Office::Dayspring, 0, 0.05),
            ..Default::default()
        },
        &crate::WorldSeed::from_json_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/demo_seed.json"
        )))
        .unwrap(),
        AreaMap::default(),
        SoundCatalog::from_toml_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../assets/sounds/catalog.toml"
        )))
        .unwrap(),
        PromptEnv::new(
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../assets/prompts/turn.j2"
            )),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../assets/prompts/night.j2"
            )),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../assets/prompts/strings.toml"
            )),
        )
        .unwrap(),
        Box::new(Unavailable),
        Box::new(NullTranscription),
        Box::new(NullTts),
        Box::new(NullSight),
        Capabilities::default(),
        (Vec3::new(0.0, 0.91, 111.0), 0.0),
        0,
        0.0,
    )
    .unwrap()
}
fn logical(n: f64) -> LogicalTime {
    LogicalTime::new(n).unwrap()
}
fn bytes(e: &Engine, now: f64) -> Admitted<Vec<u8>> {
    let b = CheckpointBudget::default();
    e.export_climate_checkpoint(logical(now), b.reserve(Cohort::SavePayload, 4096).unwrap())
        .unwrap()
        .encode()
        .unwrap()
}
fn decoded(e: &Engine, now: f64) -> Admitted<EngineClimateCandidate> {
    let b = CheckpointBudget::default();
    let saved = bytes(e, now);
    let c = ClimateCheckpointContext::from_world(&e.world, logical(now));
    EngineClimateDtoV1::decode(
        saved.value(),
        b.reserve(Cohort::LoadCandidate, saved.value().len() + 4096)
            .unwrap(),
        c,
    )
    .unwrap()
    .into_candidate(c)
    .unwrap()
}
// Test-only copy of covered authority into an independently prepared Engine.
// Other owner histories are driven identically before this seam. This is not
// production adoption or proof of a complete save/restore allocation lifetime.
fn install(candidate: &EngineClimateCandidate, e: &mut Engine) {
    let d = &candidate.data;
    // Destroy every covered field after preparing identical non-climate owner
    // histories. No poll observes this deliberately invalid intermediate state.
    e.clock = WorldClock::new(123.0, Office::Watch, 9, 0.7);
    e.config.clock = e.clock;
    e.config.weather = WeatherConfig {
        seed: 999,
        enabled: false,
        ..Default::default()
    };
    e.config.ring_the_offices = !d.ring_the_offices;
    e.config.player_id = ActorId::from_raw("wrong_recipient");
    e.weather = WeatherTimeline::new(e.config.weather);
    e.weather.set_override(WeatherKind::Fog, None, -3.0);
    e.last_weather_days = d.last_weather_days + 1.0;
    e.last_weather_sample = WeatherSample::CLEAR;
    e.last_clock_days = d.last_clock_days + 2.0;
    e.bell_strokes = VecDeque::from(vec![-1.0]);
    e.bell_seq = d.bell_seq.wrapping_add(1);
    e.world.current_time = None;
    e.world.current_weather = None;
    e.world.sounds_enabled = !d.world.sounds_enabled;
    e.clock = d.clock.clock();
    e.config.clock = d.initial_clock.clock();
    e.config.weather = d.initial_weather;
    e.config.ring_the_offices = d.ring_the_offices;
    e.config.player_id = ActorId::from_raw(&d.player_id);
    e.weather = d.weather.clone();
    e.last_weather_days = d.last_weather_days;
    e.last_weather_sample = d.last_weather_sample;
    e.last_clock_days = d.last_clock_days;
    e.bell_strokes = d.bell_strokes.clone();
    e.bell_seq = d.bell_seq;
    e.world.current_time = d.world.current_time;
    e.world.current_weather = d.world.current_weather;
    e.world.sounds_enabled = d.world.sounds_enabled;
    // Prove every covered field was restored before a poll can overwrite an
    // omitted cursor or sample and mask the missing copy in later comparisons.
    let actual = bytes(e, d.boundary.seconds());
    let expected = serde_json::to_vec(d).unwrap();
    assert_eq!(actual.value(), &expected);
}
fn climate(messages: Vec<EngineMessage>) -> Vec<EngineMessage> {
    messages
        .into_iter()
        .filter(|m| {
            matches!(
                m,
                EngineMessage::Weather(_)
                    | EngineMessage::Lightning(_)
                    | EngineMessage::Clock { .. }
            ) || matches!(m,EngineMessage::Sound{sound_id,..} if sound_id=="town_bell")
        })
        .collect()
}
fn prefix(e: &mut Engine) {
    let expected: Vec<_> = stroke_times(Office::HighWick, e.clock.elapsed_at_day(0.5))
        .filter(|t| *t > 13.0)
        .collect();
    e.poll(0.0, vec![]);
    e.poll(
        13.0,
        vec![
            EngineCommand::CycleTimeScale,
            EngineCommand::SetWeatherOverride {
                kind: WeatherKind::Thunderstorm,
                intensity: Some(0.8),
            },
        ],
    );
    assert_eq!(e.bell_strokes.iter().copied().collect::<Vec<_>>(), expected);
    assert_ne!(e.clock.scale(), e.config.clock.scale());
}
#[test]
fn checkpoint_initial_clock_provenance_does_not_extrapolate_the_unused_slope() {
    let mut e = engine();
    e.config.clock = WorldClock::new(1.0, Office::Dayspring, 0, 0.05).with_scale(0.0, 60.0);
    e.clock = e.config.clock.cycle_scale(0.0);
    e.config.weather = WeatherConfig {
        enabled: false,
        ..Default::default()
    };
    e.weather = WeatherTimeline::new(e.config.weather);
    e.world.sounds_enabled = false;
    let now = 20_000.0;
    assert!(e.config.clock.game_days(now) > checkpoint::MAX_CALENDAR_DAYS);
    assert!(e.clock.game_days(now) < checkpoint::MAX_CALENDAR_DAYS);
    // Only these component services are driven: full all-owner time horizons
    // remain a later gate, and no huge office vector is built with sound off.
    e.ring_offices(now, &mut Vec::new());
    e.update_weather(now, true, &mut Vec::new());
    let candidate = decoded(&e, now);
    assert_eq!(candidate.value().data.initial_clock.clock(), e.config.clock);
}
#[test]
fn checkpoint_climate_continues_old_slope_bells_override_and_lightning_once() {
    let mut control = engine();
    let mut resumed = engine();
    prefix(&mut control);
    prefix(&mut resumed);
    // Stop new office scheduling through the existing World switch. Old queued
    // strokes still drain; their logical deadlines must not use the new slope.
    control.world.sounds_enabled = false;
    resumed.world.sounds_enabled = false;
    let saved = decoded(&control, 13.0);
    install(saved.value(), &mut resumed);
    let mut bell_ids = std::collections::BTreeSet::new();
    let mut flash_ids = std::collections::BTreeSet::new();
    for step in 1..=450 {
        let now = 13.0 + step as f64 * 0.02;
        let expected = climate(control.poll(now, vec![]));
        let actual = climate(resumed.poll(now, vec![]));
        assert_eq!(actual, expected);
        for m in actual {
            match m {
                EngineMessage::Sound { event_id, .. } => {
                    assert!(bell_ids.insert(event_id));
                }
                EngineMessage::Lightning(s) => {
                    assert!(flash_ids.insert(s.id));
                }
                _ => {}
            }
        }
        if [25, 125, 275, 425].contains(&step) {
            assert_eq!(bytes(&control, now).value(), bytes(&resumed, now).value());
        }
    }
    assert_eq!(bell_ids.len(), 3);
    assert!(!flash_ids.is_empty());
    assert!(resumed.bell_strokes.is_empty());
    let end = decoded(&resumed, 22.0);
    assert_eq!(end.value().data.bell_seq, 4);
}
#[test]
fn checkpoint_climate_commands_resample_in_poll_order_and_clear_residue() {
    let mut control = engine();
    let mut resumed = engine();
    for e in [&mut control, &mut resumed] {
        e.poll(
            0.0,
            vec![EngineCommand::SetWeatherOverride {
                kind: WeatherKind::Downpour,
                intensity: Some(0.9),
            }],
        );
        e.poll(
            6.0,
            vec![EngineCommand::SetWeatherOverride {
                kind: WeatherKind::Clear,
                intensity: None,
            }],
        );
    }
    assert!(control.last_weather_sample.surface_wetness > 0.8);
    let saved = decoded(&control, 6.0);
    install(saved.value(), &mut resumed);
    assert_eq!(
        climate(control.poll(6.1, vec![EngineCommand::ClearWeatherOverride])),
        climate(resumed.poll(6.1, vec![EngineCommand::ClearWeatherOverride]))
    );
    assert_eq!(weather_wire::state_flags(&control.weather), (false, true));
    let residue = decoded(&control, 6.1);
    install(residue.value(), &mut resumed);
    for now in [6.2, 7.0, 8.0, 9.0, 10.0] {
        assert_eq!(
            climate(control.poll(now, vec![])),
            climate(resumed.poll(now, vec![]))
        );
        assert_eq!(bytes(&control, now).value(), bytes(&resumed, now).value());
    }
}
#[test]
fn checkpoint_climate_rejects_time_config_queue_and_numeric_corruption() {
    let mut e = engine();
    prefix(&mut e);
    let saved = bytes(&e, 13.0);
    let original: serde_json::Value = serde_json::from_slice(saved.value()).unwrap();
    let c = ClimateCheckpointContext::from_world(&e.world, logical(13.0));
    for mutate in [
        |v: &mut serde_json::Value| {
            v["world"]
                .as_object_mut()
                .unwrap()
                .remove("current_weather");
        },
        |v: &mut serde_json::Value| {
            v["world"].as_object_mut().unwrap().remove("current_time");
        },
        |v: &mut serde_json::Value| {
            v["boundary"] = json!(12.0);
        },
        |v: &mut serde_json::Value| {
            v["last_weather_days"] = json!(0.0);
        },
        |v: &mut serde_json::Value| {
            v["last_clock_days"] = json!(0.0);
        },
        |v: &mut serde_json::Value| {
            v["initial_clock"]["seconds_per_day"] = json!(61.0);
        },
        |v: &mut serde_json::Value| {
            v["clock"]["night_brightness"] = json!(0.25);
        },
        |v: &mut serde_json::Value| {
            v["initial_weather"]["seed"] = json!(999);
        },
        |v: &mut serde_json::Value| {
            v["weather"]["forced"]["began_at_days"] = json!(999.0);
        },
        |v: &mut serde_json::Value| {
            v["bell_strokes"] = json!([15.5, 14.0]);
        },
        |v: &mut serde_json::Value| {
            v["bell_strokes"] = json!([13.0]);
        },
        |v: &mut serde_json::Value| {
            v["bell_strokes"] = json!([1e100]);
        },
        |v: &mut serde_json::Value| {
            v["bell_strokes"] = json!(vec![15.5; MAX_BELL_STROKES + 1]);
        },
        |v: &mut serde_json::Value| {
            v["bell_seq"] = json!(u64::MAX);
        },
        |v: &mut serde_json::Value| {
            v["world"]["sounds_enabled"] = json!(false);
        },
        |v: &mut serde_json::Value| {
            v["player_id"] = json!("absent");
        },
    ] {
        let mut bad = original.clone();
        mutate(&mut bad);
        let bytes = serde_json::to_vec(&bad).unwrap();
        let budget = CheckpointBudget::default();
        assert!(
            EngineClimateDtoV1::decode(
                &bytes,
                budget
                    .reserve(Cohort::LoadCandidate, bytes.len() + 4096)
                    .unwrap(),
                c
            )
            .is_err()
        );
        assert_eq!(budget.retained_bytes(), 0);
    }
    let duplicate = String::from_utf8(saved.value().clone())
        .unwrap()
        .replace("\"bell_seq\":", "\"bell_seq\":0,\"bell_seq\":");
    let b = CheckpointBudget::default();
    assert!(
        EngineClimateDtoV1::decode(
            duplicate.as_bytes(),
            b.reserve(Cohort::LoadCandidate, duplicate.len() + 4096)
                .unwrap(),
            c
        )
        .unwrap_err()
        .reason
        .contains("duplicate field")
    );
}
#[test]
fn checkpoint_climate_raw_charge_and_maximum_duplicate_queue_survive_candidate() {
    let mut e = engine();
    prefix(&mut e);
    // Equal ordered times are distinct obligations. Count policy is tested at
    // its supported limit independently of how a complete clock gate will
    // constrain admission of such histories in a future full envelope.
    e.bell_strokes = VecDeque::from(vec![15.5; MAX_BELL_STROKES]);
    let saved = bytes(&e, 13.0);
    let c = ClimateCheckpointContext::from_world(&e.world, logical(13.0));
    let mut padded = saved.value().clone();
    padded.extend(std::iter::repeat_n(b' ', 1024 * 1024));
    let b = CheckpointBudget::default();
    let decoded = EngineClimateDtoV1::decode(
        &padded,
        b.reserve(Cohort::LoadCandidate, padded.len() + 4096)
            .unwrap(),
        c,
    )
    .unwrap();
    let cost = decoded.value().cost().unwrap();
    assert!(decoded.reserved_bytes() >= cost.peak_bytes + 3 * 1024 * 1024);
    let charge = decoded.reserved_bytes();
    let candidate = decoded.into_candidate(c).unwrap();
    assert_eq!(candidate.reserved_bytes(), charge);
    assert_eq!(candidate.value().counts(c).bell_strokes, MAX_BELL_STROKES);
    drop(candidate);
    assert_eq!(b.retained_bytes(), 0);
    for (payload, reason) in [
        (
            format!("{{\"bad\":{}0{}}}", "[".repeat(70), "]".repeat(70)),
            "nesting",
        ),
        (
            format!(
                "{{\"bad\":[{}]}}",
                "[],".repeat(270_000).trim_end_matches(',')
            ),
            "aggregate expanded",
        ),
    ] {
        let error = EngineClimateDtoV1::decode(
            payload.as_bytes(),
            b.reserve(Cohort::LoadCandidate, payload.len() + 4096)
                .unwrap(),
            c,
        )
        .unwrap_err();
        assert!(error.reason.contains(reason));
        assert_eq!(b.retained_bytes(), 0);
    }
}
#[test]
fn checkpoint_climate_layout_and_supported_fixtures_are_exact() {
    for (name, n, limit) in [
        (
            "EngineClimateDtoV1",
            std::mem::size_of::<EngineClimateDtoV1>(),
            1024,
        ),
        ("EngineWire", std::mem::size_of::<EngineWire>(), 1024),
        ("EngineView", std::mem::size_of::<EngineView>(), 768),
        ("WorldClimateV1", std::mem::size_of::<WorldClimateV1>(), 160),
    ] {
        println!("{name}={n}");
        assert!(n <= limit);
    }
    // Queue floats/VecDeque growth use <=16 bytes per row, below their 64-byte
    // scalar charge. Strings pay headers plus UTF8 bytes; no owner BTree index
    // or internally tagged Content buffer is needed by these closed records.
    let mut e = engine();
    let initial = bytes(&e, 0.0);
    prefix(&mut e);
    let active = bytes(&e, 13.0);
    assert_eq!(
        initial.value().as_slice(),
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/checkpoint_v1/climate_initial.json"
        ))
    );
    assert_eq!(
        active.value().as_slice(),
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/checkpoint_v1/climate_active.json"
        ))
    );
    let b = CheckpointBudget::default();
    let virgin = World::new()
        .export_climate_checkpoint(logical(0.0), b.reserve(Cohort::SavePayload, 4096).unwrap())
        .unwrap()
        .encode()
        .unwrap();
    assert_eq!(
        virgin.value().as_slice(),
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/checkpoint_v1/world_climate_virgin.json"
        ))
    );
}
#[test]
#[ignore = "explicit supported M2a4 component fixture writer"]
fn write_checkpoint_climate_fixtures() {
    let root =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/checkpoint_v1");
    let mut e = engine();
    std::fs::write(root.join("climate_initial.json"), bytes(&e, 0.0).value()).unwrap();
    prefix(&mut e);
    std::fs::write(root.join("climate_active.json"), bytes(&e, 13.0).value()).unwrap();
    let b = CheckpointBudget::default();
    let virgin = World::new()
        .export_climate_checkpoint(logical(0.0), b.reserve(Cohort::SavePayload, 4096).unwrap())
        .unwrap()
        .encode()
        .unwrap();
    std::fs::write(root.join("world_climate_virgin.json"), virgin.value()).unwrap();
}
