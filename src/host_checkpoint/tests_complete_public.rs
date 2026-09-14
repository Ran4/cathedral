//! Coordinator-owned complete-checkpoint tests through the ordinary host seam.
//! The shared small host harness has no OS window, renderer, audio or provider.

use super::{HostCaptureSet, capture_complete, validate_complete};
use bevy::prelude::*;
use cathedral_sim::checkpoint::{
    Admitted, CheckpointBudget, Cohort, MAX_RESIDENT_BYTES, Reservation,
    complete::{
        CheckpointCategory, CheckpointProfile, CompleteCheckpointCandidate,
        CompleteCheckpointInput, RUNNING_AUTHORITY_ALLOWANCE_BYTES,
    },
};
use serde_json::{Value, json};

type Check = Box<dyn FnOnce(&World) -> Vec<u8> + Send + Sync>;

#[derive(Resource)]
struct Probe {
    check: Option<Check>,
    result: Option<Vec<u8>>,
}

fn app() -> App {
    let mut app = super::tests_public::app();
    app.add_systems(PostUpdate, inspect.in_set(HostCaptureSet));
    app
}

fn inspect(world: &mut World) {
    let Some(mut probe) = world.get_resource_mut::<Probe>() else {
        return;
    };
    let Some(check) = probe.check.take() else {
        return;
    };
    drop(probe);
    let before = super::tests_public::simulation_stamp(world);
    let result = check(world);
    assert_eq!(super::tests_public::simulation_stamp(world), before);
    world.resource_mut::<Probe>().result = Some(result);
}

fn boundary(
    app: &mut App,
    check: impl FnOnce(&World) -> Vec<u8> + Send + Sync + 'static,
) -> Vec<u8> {
    app.insert_resource(Probe {
        check: Some(Box::new(check)),
        result: None,
    });
    app.update();
    app.world_mut()
        .remove_resource::<Probe>()
        .unwrap()
        .result
        .expect("ordinary complete capture point did not execute")
}

fn running(budget: &CheckpointBudget) -> Reservation {
    budget
        .reserve(Cohort::Running, RUNNING_AUTHORITY_ALLOWANCE_BYTES)
        .unwrap()
}

fn save(world: &World, budget: &CheckpointBudget) -> Admitted<CompleteCheckpointCandidate> {
    capture_complete(
        world,
        CheckpointProfile::Authored,
        budget.reserve(Cohort::SavePayload, 4096).unwrap(),
    )
    .unwrap()
}

fn input(bytes: &[u8], budget: &CheckpointBudget) -> CompleteCheckpointInput {
    CompleteCheckpointInput::copy_from(
        bytes,
        budget
            .reserve(Cohort::LoadCandidate, bytes.len() * 2 + 4096)
            .unwrap(),
    )
    .unwrap()
}

fn reject(world: &World, bytes: &[u8], budget: &CheckpointBudget, label: &str) {
    let before = budget.retained_bytes();
    let result = validate_complete(world, input(bytes, budget));
    assert!(
        result.is_err(),
        "accepted malformed complete checkpoint: {label}"
    );
    assert_eq!(
        budget.retained_bytes(),
        before,
        "failed load retained a charge: {label}"
    );
}

#[test]
fn complete_capture_repeats_exactly_and_candidate_keeps_original_category_slices() {
    boundary(&mut app(), |world| {
        let budget = CheckpointBudget::default();
        let running = running(&budget);
        let saved = save(world, &budget);
        let raw = saved.value().bytes();
        let wire: Value = serde_json::from_slice(raw).unwrap();
        let loaded = validate_complete(world, input(raw, &budget)).unwrap();
        assert_eq!(loaded.value().bytes(), raw);
        assert_eq!(
            loaded.value().world_identity(),
            saved.value().world_identity()
        );
        assert_eq!(loaded.value().boundary(), saved.value().boundary());
        let categories = [
            (CheckpointCategory::Ledger, "ledger"),
            (CheckpointCategory::Operations, "operations"),
            (CheckpointCategory::Backbone, "backbone"),
            (CheckpointCategory::Round, "round"),
            (CheckpointCategory::Climate, "climate"),
            (CheckpointCategory::Knowledge, "knowledge"),
            (CheckpointCategory::Law, "law"),
            (CheckpointCategory::Marks, "marks"),
            (CheckpointCategory::Animals, "animals"),
            (CheckpointCategory::Social, "social"),
            (CheckpointCategory::Continuity, "continuity"),
            (CheckpointCategory::Scheduler, "scheduler"),
            (CheckpointCategory::Night, "night"),
            (CheckpointCategory::Speech, "speech"),
            (CheckpointCategory::CognitionInputs, "cognition_inputs"),
            (CheckpointCategory::Host, "host"),
        ];
        let owned = loaded.value().bytes();
        for (category, key) in categories {
            let part = loaded.value().category(category);
            assert!(!part.is_empty());
            assert!(part.as_ptr() >= owned.as_ptr());
            assert!(part.as_ptr_range().end <= owned.as_ptr_range().end);
            assert_eq!(serde_json::from_slice::<Value>(part).unwrap(), wire[key]);
        }
        let original = raw.to_vec();
        drop(saved);
        assert!(budget.reserve(Cohort::LoadCandidate, 1).is_err());
        let repeated = save(world, &budget);
        assert_eq!(repeated.value().bytes(), original);
        assert_eq!(loaded.value().bytes(), original);
        drop(repeated);
        assert!(budget.retained_bytes() > running.bytes());
        std::thread::spawn(move || drop(loaded)).join().unwrap();
        assert_eq!(budget.retained_bytes(), running.bytes());
        drop(running);
        assert_eq!(budget.retained_bytes(), 0);
        original
    });
}

#[test]
fn complete_outer_schema_requires_every_field_and_rejects_duplicates_and_nulls() {
    boundary(&mut app(), |world| {
        let budget = CheckpointBudget::default();
        let _running = running(&budget);
        let saved = save(world, &budget);
        let original: Value = serde_json::from_slice(saved.value().bytes()).unwrap();
        let canonical = serde_json::to_string(&original).unwrap();
        assert!(canonical.starts_with('{'));
        let fields = original.as_object().unwrap();
        assert_eq!(fields.len(), 21);
        for (key, value) in fields {
            let mut changed = original.clone();
            changed.as_object_mut().unwrap().remove(key);
            reject(world, &serde_json::to_vec(&changed).unwrap(), &budget, key);
            let mut changed = original.clone();
            changed[key] = Value::Null;
            reject(world, &serde_json::to_vec(&changed).unwrap(), &budget, key);
            let duplicate = format!(
                "{{{}:{},{}",
                serde_json::to_string(key).unwrap(),
                value,
                &canonical[1..]
            );
            reject(world, duplicate.as_bytes(), &budget, key);
        }
        let mut changed = original.clone();
        changed["unknown_complete_field"] = json!(true);
        reject(
            world,
            &serde_json::to_vec(&changed).unwrap(),
            &budget,
            "unknown field",
        );
        changed = original.clone();
        changed["version"] = json!(u16::MAX);
        reject(
            world,
            &serde_json::to_vec(&changed).unwrap(),
            &budget,
            "future version",
        );
        changed = original;
        changed["profile"] = json!("unbounded");
        reject(
            world,
            &serde_json::to_vec(&changed).unwrap(),
            &budget,
            "unknown profile",
        );
        changed["profile"] = json!({"authored": null});
        reject(
            world,
            &serde_json::to_vec(&changed).unwrap(),
            &budget,
            "profile must use its string spelling",
        );
        saved.value().bytes().to_vec()
    });
}

#[test]
fn earlier_complete_payload_validates_after_live_simulation_has_advanced() {
    let mut app = app();
    let earlier = boundary(&mut app, |world| {
        let budget = CheckpointBudget::default();
        let _running = running(&budget);
        save(world, &budget).value().bytes().to_vec()
    });
    boundary(&mut app, move |world| {
        let budget = CheckpointBudget::default();
        let _running = running(&budget);
        let current = save(world, &budget);
        assert_ne!(current.value().bytes(), earlier);
        let old = validate_complete(world, input(&earlier, &budget)).unwrap();
        assert_eq!(old.value().bytes(), earlier);
        assert_ne!(old.value().boundary(), current.value().boundary());
        drop(old);
        let a: Value = serde_json::from_slice(&earlier).unwrap();
        let b: Value = serde_json::from_slice(current.value().bytes()).unwrap();
        for category in ["backbone", "climate", "animals", "scheduler", "host"] {
            assert_ne!(
                a[category], b[category],
                "scenario did not change {category}"
            );
            let mut mixed = a.clone();
            mixed[category] = b[category].clone();
            reject(
                world,
                &serde_json::to_vec(&mixed).unwrap(),
                &budget,
                category,
            );
        }
        earlier
    });
}

#[test]
fn consuming_input_checks_owned_capacity_and_releases_refused_ownership() {
    let budget = CheckpointBudget::default();
    let mut bytes = Vec::with_capacity(1024 * 1024);
    bytes.extend_from_slice(b"{}");
    let lease = budget
        .reserve(Cohort::LoadCandidate, bytes.len() + 4096)
        .unwrap();
    assert!(CompleteCheckpointInput::from_owned(bytes, lease).is_err());
    assert_eq!(budget.retained_bytes(), 0);
    let lease = budget.reserve(Cohort::SavePayload, 4096).unwrap();
    assert!(CompleteCheckpointInput::from_owned(b"{}".to_vec(), lease).is_err());
    assert_eq!(budget.retained_bytes(), 0);
}

#[test]
fn compatible_fresh_host_accepts_saved_lineage_but_changed_installed_geometry_does_not() {
    let saved = boundary(&mut app(), |world| {
        let budget = CheckpointBudget::default();
        let _running = running(&budget);
        save(world, &budget).value().bytes().to_vec()
    });
    let original = saved.clone();
    boundary(&mut app(), move |world| {
        let budget = CheckpointBudget::default();
        let _running = running(&budget);
        let current = save(world, &budget);
        let loaded = validate_complete(world, input(&original, &budget)).unwrap();
        assert_ne!(
            loaded.value().world_identity(),
            current.value().world_identity()
        );
        assert_eq!(loaded.value().bytes(), original);
        original
    });
    let mut changed = app();
    changed
        .world_mut()
        .resource_mut::<crate::controller::CollisionWorld>()
        .add_box(
            Vec3::new(1000.0, 0.0, 1000.0),
            Vec3::new(1001.0, 1.0, 1001.0),
        );
    boundary(&mut changed, move |world| {
        let budget = CheckpointBudget::default();
        let _running = running(&budget);
        reject(
            world,
            &saved,
            &budget,
            "changed actual installed collision geometry",
        );
        saved
    });
}

#[test]
fn complete_loader_retains_original_padding_and_shared_pressure_refuses_staging() {
    boundary(&mut app(), |world| {
        let budget = CheckpointBudget::default();
        let running = running(&budget);
        let saved = save(world, &budget);
        let mut padded = vec![b' '; 1024 * 1024];
        padded.extend_from_slice(saved.value().bytes());
        padded.extend_from_slice(b"\n\t ");
        let loaded = validate_complete(world, input(&padded, &budget)).unwrap();
        assert_eq!(loaded.value().bytes(), padded);
        assert!(loaded.reserved_bytes() >= padded.len());
        drop(loaded);

        let base = budget.retained_bytes();
        let input_bytes = saved.value().bytes().len() * 2 + 4096;
        let retiring = budget
            .reserve(
                Cohort::RetiringGeneration,
                MAX_RESIDENT_BYTES - base - input_bytes,
            )
            .unwrap();
        reject(
            world,
            saved.value().bytes(),
            &budget,
            "aggregate staging pressure",
        );
        assert_eq!(budget.retained_bytes(), base + retiring.bytes());
        assert!(budget.peak_retained_bytes() <= MAX_RESIDENT_BYTES);
        drop(retiring);
        drop(saved);
        assert_eq!(budget.retained_bytes(), running.bytes());
        padded
    });
}

#[test]
fn complete_capture_requires_running_authority_in_the_same_shared_budget() {
    boundary(&mut app(), |world| {
        let unrelated = CheckpointBudget::default();
        let _elsewhere = running(&unrelated);
        let budget = CheckpointBudget::default();
        let result = capture_complete(
            world,
            CheckpointProfile::Authored,
            budget.reserve(Cohort::SavePayload, 4096).unwrap(),
        );
        assert!(
            result.is_err(),
            "a different budget cannot supply the Running charge"
        );
        assert_eq!(budget.retained_bytes(), 0);
        let _running = running(&budget);
        let saved = save(world, &budget);
        saved.value().bytes().to_vec()
    });
}

#[test]
fn complete_rate_check_rejects_unsafe_slope_with_consistent_saved_calendar_cursors() {
    boundary(&mut app(), |world| {
        use cathedral_sim::engine::climate_checkpoint::{
            ClimateCheckpointContext, EngineClimateDtoV1,
        };
        let budget = CheckpointBudget::default();
        let _running = running(&budget);
        let saved = save(world, &budget);
        let now = saved.value().boundary();
        let mut changed: Value = serde_json::from_slice(saved.value().bytes()).unwrap();
        let days = changed["climate"]["last_clock_days"].clone();
        let clock = &mut changed["climate"]["clock"];
        clock["elapsed_origin"] = json!(now.seconds());
        clock["epoch_days"] = days;
        clock["scale"] = json!(1_000_000.0);
        let clock = clock.clone();
        assert_eq!(changed["night"]["night"]["context"]["clock"]["version"], 1);
        changed["night"]["night"]["context"]["clock"] = clock;
        changed["host"]["scalars"]["clock"]["scale"] = json!(1_000_000.0);

        // The legacy climate contract admits this precise current position.
        // It is the complete next-consumer horizon that must reject the slope.
        let bytes = serde_json::to_vec(&changed["climate"]).unwrap();
        let engine = world
            .non_send::<crate::smart_actors::local_engine::LocalEngine>()
            .checkpoint_engine()
            .unwrap();
        let context = ClimateCheckpointContext::from_world(engine.world(), now);
        let component = EngineClimateDtoV1::decode(
            &bytes,
            budget
                .reserve(Cohort::LoadCandidate, bytes.len() + 4096)
                .unwrap(),
            context,
        )
        .expect("the corruption must preserve the component's calendar agreement");
        drop(component);
        let bytes = serde_json::to_vec(&changed).unwrap();
        let before = budget.retained_bytes();
        let error = validate_complete(world, input(&bytes, &budget)).unwrap_err();
        assert_eq!(error.owner, "complete");
        assert!(
            error.reason.contains("rate") || error.reason.contains("horizon"),
            "unsafe slope failed for another reason: {error}"
        );
        assert_eq!(budget.retained_bytes(), before);
        saved.value().bytes().to_vec()
    });
}

fn digest_paths(value: &Value, path: &str, paths: &mut Vec<String>) {
    match value {
        Value::Array(values)
            if values.len() == 32
                && values.iter().all(|v| v.as_u64().is_some_and(|n| n <= 255)) =>
        {
            paths.push(path.to_owned());
        }
        Value::Object(fields) => {
            for (key, value) in fields {
                let key = key.replace('~', "~0").replace('/', "~1");
                digest_paths(value, &format!("{path}/{key}"), paths);
            }
        }
        _ => {}
    }
}

#[test]
fn every_saved_compatibility_digest_and_manifest_field_is_bound() {
    boundary(&mut app(), |world| {
        let budget = CheckpointBudget::default();
        let _running = running(&budget);
        let saved = save(world, &budget);
        let original: Value = serde_json::from_slice(saved.value().bytes()).unwrap();
        let mut paths = Vec::new();
        digest_paths(&original["manifest"], "/manifest", &mut paths);
        assert!(
            paths.len() >= 15,
            "exercise production and installed-definition roles"
        );
        for path in paths {
            let mut changed = original.clone();
            let digest = changed.pointer_mut(&path).unwrap().as_array_mut().unwrap();
            digest[0] = json!(digest[0].as_u64().unwrap() ^ 1);
            reject(
                world,
                &serde_json::to_vec(&changed).unwrap(),
                &budget,
                &path,
            );
        }
        for key in original["manifest"].as_object().unwrap().keys() {
            let mut changed = original.clone();
            changed["manifest"].as_object_mut().unwrap().remove(key);
            reject(world, &serde_json::to_vec(&changed).unwrap(), &budget, key);
        }
        let mut changed = original;
        changed["manifest"]["toolchain"] = json!("a".repeat(4097));
        reject(
            world,
            &serde_json::to_vec(&changed).unwrap(),
            &budget,
            "toolchain text bound",
        );
        saved.value().bytes().to_vec()
    });
}

#[test]
fn complete_host_rows_keep_the_existing_canonical_contract_with_a_timed_hud_message() {
    use cathedral_sim::checkpoint::host::export_host_checkpoint;
    let mut app = app();
    app.world_mut()
        .resource_mut::<crate::smart_actors::hud::SmartActorHudState>()
        .toast("A readable message present at the ordinary boundary.");
    boundary(&mut app, |world| {
        let budget = CheckpointBudget::default();
        let _running = running(&budget);
        let saved = save(world, &budget);
        let host_bytes = saved.value().category(CheckpointCategory::Host).to_vec();
        let host: Value = serde_json::from_slice(&host_bytes).unwrap();
        assert!(host["records"].as_array().unwrap().iter().any(|row| {
            row["kind"] == "hud"
                && row["data"]["slot"] == "transient"
                && row["data"]["text"] == "A readable message present at the ordinary boundary."
        }));
        let original = saved.value().bytes().to_vec();
        drop(saved);
        let source = super::HostObservation::new(world).unwrap();
        let component = export_host_checkpoint(
            &source,
            source.context().unwrap(),
            budget.reserve(Cohort::SavePayload, 4096).unwrap(),
        )
        .unwrap()
        .encode()
        .unwrap();
        assert_eq!(component.value(), &host_bytes);
        original
    });
}

#[test]
fn complete_round_requires_seeded_office_cursor_and_bounded_consumer_spans() {
    boundary(&mut app(), |world| {
        use cathedral_sim::round::checkpoint::{RoundCheckpointContext, RoundDtoV1};
        let budget = CheckpointBudget::default();
        let _running = running(&budget);
        let saved = save(world, &budget);
        let original: Value = serde_json::from_slice(saved.value().bytes()).unwrap();
        assert_eq!(original["round"]["round"]["seeded"], true);
        assert!(original["round"]["round"]["last_office_days"].is_number());
        let engine = world
            .non_send::<crate::smart_actors::local_engine::LocalEngine>()
            .checkpoint_engine()
            .unwrap();
        let context =
            RoundCheckpointContext::from_world(engine.world(), engine.config().nav.as_deref());
        for (key, value) in [
            ("last_office_days", Value::Null),
            ("last_office_days", json!(-100_000.0)),
            ("last_game_days", json!(-100_000.0)),
            ("production_last_game_days", json!(-100_000.0)),
        ] {
            let mut changed = original.clone();
            changed["round"]["round"][key] = value;
            let component_bytes = serde_json::to_vec(&changed["round"]).unwrap();
            let component = RoundDtoV1::decode(
                &component_bytes,
                budget
                    .reserve(Cohort::LoadCandidate, component_bytes.len() + 4096)
                    .unwrap(),
                context,
            )
            .expect("legacy Round accepts individually bounded or absent cursors");
            drop(component);
            let bytes = serde_json::to_vec(&changed).unwrap();
            reject(world, &bytes, &budget, key);
        }
        saved.value().bytes().to_vec()
    });
}

#[test]
fn complete_outer_errors_do_not_echo_oversized_input() {
    boundary(&mut app(), |world| {
        let budget = CheckpointBudget::default();
        let _running = running(&budget);
        let saved = save(world, &budget);
        let original: Value = serde_json::from_slice(saved.value().bytes()).unwrap();
        let oversized = "x".repeat(1024 * 1024);
        let root_string = serde_json::to_vec(&oversized).unwrap();
        let mut unknown_key = serde_json::Map::new();
        unknown_key.insert(oversized.clone(), Value::Null);
        let unknown_key = serde_json::to_vec(&unknown_key).unwrap();
        let mut metadata = original;
        metadata["version"] = json!(oversized);
        let metadata = serde_json::to_vec(&metadata).unwrap();
        for (label, bytes) in [
            ("root string", root_string),
            ("object key", unknown_key),
            ("metadata value", metadata),
        ] {
            let before = budget.retained_bytes();
            let error = validate_complete(world, input(&bytes, &budget)).unwrap_err();
            assert!(error.reason.len() < 1024, "unbounded {label} diagnostic");
            assert_eq!(budget.retained_bytes(), before);
        }
        saved.value().bytes().to_vec()
    });
}
