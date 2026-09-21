//! Owner roundtrip/retention measurements using actual CityPlugin boundaries.
use super::*;
use cathedral_sim::checkpoint::{
    self, CheckpointBudget, Cohort,
    complete::{
        self, CheckpointCategory, CheckpointProfile, CompleteCheckpointInput, HydrationAssets,
        HydrationWorldAssets,
    },
};
use cathedral_sim::{Engine, EngineConfig, PromptEnv, RuntimeGeneration};
use std::{path::PathBuf, sync::Arc, time::Instant};

// Diagnostic-only allocator: no change to the production executable allocator.
// Cumulative requested bytes include every allocation/reallocation on this
// thread during the factory, even if immediately freed. Factory code is wholly
// synchronous; backend threads belong to the separately admitted Running host.
struct FactoryAllocator;
std::thread_local! {
    static FACTORY_ALLOCATED: std::cell::Cell<Option<usize>> = const { std::cell::Cell::new(None) };
}
fn count_factory_allocation(bytes: usize) {
    let _ = FACTORY_ALLOCATED.try_with(|n| {
        if let Some(value) = n.get() {
            n.set(Some(value.saturating_add(bytes)));
        }
    });
}
unsafe impl std::alloc::GlobalAlloc for FactoryAllocator {
    unsafe fn alloc(&self, layout: std::alloc::Layout) -> *mut u8 {
        count_factory_allocation(layout.size());
        unsafe { std::alloc::System.alloc(layout) }
    }
    unsafe fn alloc_zeroed(&self, layout: std::alloc::Layout) -> *mut u8 {
        count_factory_allocation(layout.size());
        unsafe { std::alloc::System.alloc_zeroed(layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: std::alloc::Layout, size: usize) -> *mut u8 {
        count_factory_allocation(size);
        unsafe { std::alloc::System.realloc(ptr, layout, size) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: std::alloc::Layout) {
        unsafe { std::alloc::System.dealloc(ptr, layout) }
    }
}
#[global_allocator]
static ALLOCATOR: FactoryAllocator = FactoryAllocator;
pub(super) fn factory_allocations<T>(f: impl FnOnce() -> T) -> (T, usize) {
    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            FACTORY_ALLOCATED.with(|n| n.set(None));
        }
    }
    FACTORY_ALLOCATED.with(|n| {
        assert!(n.get().is_none());
        n.set(Some(0));
    });
    let guard = Reset;
    let value = f();
    let bytes = FACTORY_ALLOCATED.with(|n| n.get().unwrap());
    drop(guard);
    (value, bytes)
}

/// Setup only; independent assertions belong to tests_hydration_public. Caller
/// invokes this only inside the factory covered by prepare_hydration's lease.
pub(super) fn installed_assets(e: &Engine, extra: u32) -> checkpoint::Result<HydrationAssets> {
    installed_assets_with(e, extra, |_, _| {})
}
pub(super) fn installed_assets_with(
    e: &Engine,
    extra: u32,
    modify: impl FnOnce(&mut EngineConfig, &mut HydrationWorldAssets),
) -> checkpoint::Result<HydrationAssets> {
    let error = |e: String| checkpoint::CheckpointError {
        owner: "hydration_fixture",
        reason: e,
    };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let assets = root.join("assets");
    let read = |p: &str| std::fs::read_to_string(assets.join(p)).map_err(|e| error(e.to_string()));
    let mut seed = cathedral_backends::world_data::load_world_seed(&assets, &root.join("lore"))
        .map_err(error)?;
    if extra != 0 {
        // Original ordered definition input, not World/Engine/Round creation.
        // Exactly mirrors local_engine::with_extra_ambient's deterministic input.
        let occupied: Vec<_> = seed.characters.iter().map(|c| c.position_m).collect();
        let crowd = cathedral_sim::generate_ambient(
            e.config().nav.as_deref().unwrap(),
            extra as usize,
            0,
            &occupied,
            &[],
        )
        .map_err(error)?;
        seed = seed
            .with_extra_ambient(crowd.sheets)
            .map_err(|e| error(e.to_string()))?;
    }
    let env = PromptEnv::new(
        &read("prompts/turn.j2")?,
        &read("prompts/night.j2")?,
        &read("prompts/strings.toml")?,
    )
    .map_err(|e| error(e.to_string()))?;
    let w = e.world();
    let mut config = e.config().clone();
    // Deep-copy every non-navigation immutable role while the factory counter
    // is active. Navigation is the sole inherited allocation; its distinct Arc
    // identities and current retained capacities are charged separately below.
    config.shelters = Arc::new((*e.config().shelters).clone());
    let mut world = HydrationWorldAssets {
        areas: w.area_map.clone(),
        sounds: w.sound_catalog.clone(),
        items: Arc::new((*w.item_catalog).clone()),
        nav: w.nav.clone(),
        shelters: Arc::new((*w.shelters).clone()),
        marks: Arc::new((*w.mark_catalog).clone()),
        facts: Arc::new((*w.fact_catalog).clone()),
        salience: Arc::new((*w.salience).clone()),
        area_adjacency: Arc::new((*w.area_adjacency).clone()),
    };
    modify(&mut config, &mut world);
    HydrationAssets::new(&seed, config, env, world)
}

pub(super) const CATEGORIES: [CheckpointCategory; 16] = [
    CheckpointCategory::Ledger,
    CheckpointCategory::Operations,
    CheckpointCategory::Backbone,
    CheckpointCategory::Round,
    CheckpointCategory::Climate,
    CheckpointCategory::Knowledge,
    CheckpointCategory::Law,
    CheckpointCategory::Marks,
    CheckpointCategory::Animals,
    CheckpointCategory::Social,
    CheckpointCategory::Continuity,
    CheckpointCategory::Scheduler,
    CheckpointCategory::Night,
    CheckpointCategory::Speech,
    CheckpointCategory::CognitionInputs,
    CheckpointCategory::Host,
];
pub(super) const ASSETS_UPPER_BYTES: usize = 64 * 1024 * 1024;
// MiniJinja2.21 shared builtins/integer/default-syntax caches and bounded
// compiler TLS pools: source-derived712,704B, conservatively rounded to1MiB.
// See coordinator/prompt-shared-bound.json. Process/TLS owners remain under
// the caller's Running scope after the hydrated candidate is disposed.
const SHARED_PROMPT_RUNTIME_UPPER_BYTES: usize = 1024 * 1024;

fn retained_shared_nav_bytes(engine: &Engine) -> usize {
    let world = engine.world().nav.as_ref();
    let config = engine.config().nav.as_ref();
    let retained = |nav: &Arc<cathedral_sim::NavData>| {
        let n = nav.checkpoint_storage_inventory();
        // Inventory includes NavData itself; add outer Arc control/alignment.
        n.graph_and_indexes_bytes + n.cache_retained_bytes + 64
    };
    world.map_or(0, retained)
        + config
            .filter(|config| world.is_none_or(|world| !Arc::ptr_eq(world, config)))
            .map_or(0, retained)
}

#[derive(Resource)]
struct Probe {
    extra: u32,
    samples: usize,
    pending_case: u8,
    report: Option<serde_json::Value>,
}
fn inspect(world: &mut World) {
    let Some(probe) = world.get_resource::<Probe>() else {
        return;
    };
    if probe.report.is_some() {
        return;
    }
    let (extra, samples, pending_case) = (probe.extra, probe.samples, probe.pending_case);
    let before = super::tests_public::simulation_stamp(world);
    let source = HostObservation::new(world).unwrap();
    let definitions = source.scalars().unwrap().definitions;
    let generation = RuntimeGeneration(source.scalars().unwrap().boundary.generation)
        .successor()
        .unwrap();
    let engine = source.local().unwrap().checkpoint_engine().unwrap();
    if pending_case == 2 {
        let check_budget = CheckpointBudget::default();
        let input = engine
            .export_cognition_inputs_checkpoint(
                cathedral_sim::timeline::LogicalTime::new(
                    source.scalars().unwrap().time.virtual_elapsed.as_secs_f64(),
                )
                .unwrap(),
                check_budget.reserve(Cohort::SavePayload, 4096).unwrap(),
            )
            .unwrap();
        if !input.value().counts().night {
            return;
        }
    }
    let budget = CheckpointBudget::default();
    let running = budget
        .reserve(Cohort::Running, complete::RUNNING_AUTHORITY_ALLOWANCE_BYTES)
        .unwrap();
    let mut times = Vec::with_capacity(samples);
    let mut stages = Vec::with_capacity(samples);
    let mut costs = Vec::with_capacity(samples);
    let mut capture_costs = Vec::with_capacity(samples);
    let mut factory_bytes = Vec::with_capacity(samples);
    let mut pending_counts = Vec::with_capacity(samples);
    let mut protected_speech_counts = Vec::with_capacity(samples);
    let nav_before = engine
        .world()
        .nav
        .as_ref()
        .map(|n| n.checkpoint_storage_inventory());
    let config_nav_before = engine
        .config()
        .nav
        .as_ref()
        .map(|n| n.checkpoint_storage_inventory());
    let nav_shared = engine
        .world()
        .nav
        .as_ref()
        .zip(engine.config().nav.as_ref())
        .is_some_and(|(a, b)| Arc::ptr_eq(a, b));
    let shared_nav_bytes = retained_shared_nav_bytes(engine);
    for _ in 0..samples {
        let saved = capture_complete(
            world,
            if extra == 0 {
                CheckpointProfile::Authored
            } else {
                CheckpointProfile::Populated
            },
            budget.reserve(Cohort::SavePayload, 4096).unwrap(),
        )
        .unwrap();
        use sha2::{Digest, Sha256};
        let digests: [[u8; 32]; 16] =
            CATEGORIES.map(|c| Sha256::digest(saved.value().category(c)).into());
        let load = validate_complete(
            world,
            CompleteCheckpointInput::copy_from(
                saved.value().bytes(),
                budget.reserve(Cohort::LoadCandidate, 4096).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
        let scheduler: serde_json::Value =
            serde_json::from_slice(saved.value().category(CheckpointCategory::Scheduler)).unwrap();
        let night: serde_json::Value =
            serde_json::from_slice(saved.value().category(CheckpointCategory::Night)).unwrap();
        let speech: serde_json::Value =
            serde_json::from_slice(saved.value().category(CheckpointCategory::Speech)).unwrap();
        let pending = serde_json::json!({
            "scheduler_in_flight":!scheduler["scheduler"]["in_flight"].is_null(),
            "scheduler_held":!scheduler["scheduler"]["held_result"].is_null(),
            "night_in_flight":!night["night"]["night"]["in_flight"].is_null(),
            "night_held":!night["night"]["night"]["held_result"].is_null(),
            "speech_captures":speech["state"]["captures"].as_array().map_or(0,Vec::len),
            "speech_streams":speech["state"]["streams"].as_array().map_or(0,Vec::len),
            "accepted_recordings":speech["state"]["accepted_recordings"].as_array().map_or(0,Vec::len),
        });
        if pending_case == 1 {
            assert!(
                pending["scheduler_held"].as_bool().unwrap(),
                "nonempty held scheduler fixture: {scheduler}"
            );
            assert!(
                pending["speech_streams"].as_u64().unwrap() > 0,
                "nonempty interrupted speech fixture: {speech}"
            );
        }
        if pending_case == 3 {
            assert!(
                pending["accepted_recordings"].as_u64().unwrap() > 0,
                "ordinary recording must own its accepted receipt: {speech}; ledger={}",
                String::from_utf8_lossy(saved.value().category(CheckpointCategory::Ledger))
            );
        }
        pending_counts.push(pending);
        drop((scheduler, night, speech));
        capture_costs.push(saved.value().cost());
        let lineage = saved.value().world_identity();
        let boundary = saved.value().boundary();
        drop(saved);
        let prepared = load.prepare_hydration(ASSETS_UPPER_BYTES).unwrap();
        let start = Instant::now();
        let mut previous = start;
        let mut checkpoints = Vec::with_capacity(6);
        let mut factory_allocated = 0;
        let hydrated = prepared
            .hydrate_observed(
                || {
                    let (assets, allocated) =
                        factory_allocations(|| installed_assets(engine, extra));
                    factory_allocated = allocated;
                    assets
                },
                definitions,
                generation,
                &mut |stage| {
                    let now = Instant::now();
                    checkpoints.push((stage, now.duration_since(previous).as_secs_f64() * 1e6));
                    previous = now;
                },
            )
            .unwrap();
        let hydration_micros = start.elapsed().as_secs_f64() * 1e6;
        assert!(
            factory_allocated + shared_nav_bytes + SHARED_PROMPT_RUNTIME_UPPER_BYTES
                <= ASSETS_UPPER_BYTES
        );
        assert_eq!(hydrated.value().world_identity(), lineage);
        assert_eq!(hydrated.value().boundary(), boundary);
        assert_eq!(hydrated.value().runtime_generation(), generation);
        assert_eq!(
            hydrated.value().character_count(),
            engine.world().characters.len()
        );
        assert_eq!(hydrated.value().item_count(), engine.world().items.len());
        if pending_case == 2 {
            let inputs = hydrated.value().cognition_inputs();
            assert!(
                inputs.night().is_some(),
                "submitted Night obligation must survive"
            );
            assert!(!inputs.night().unwrap().prompt().is_empty());
        }
        if pending_case == 1 {
            assert!(!hydrated.value().speech().streams().is_empty());
            assert!(hydrated.value().cognition_inputs().scheduler().is_some());
        }
        assert_eq!(
            hydrated.value().protected_speech_action_count(),
            hydrated.value().speech().semantic_ids().count()
        );
        assert!(
            hydrated
                .value()
                .speech()
                .semantic_ids()
                .all(|id| hydrated.value().protects_speech_action(id))
        );
        if pending_case == 3 {
            assert!(hydrated.value().protected_speech_action_count() > 0);
        }
        protected_speech_counts.push(hydrated.value().protected_speech_action_count());
        for (category, expected) in CATEGORIES.into_iter().zip(digests) {
            let digest = hydrated
                .value()
                .category_digest(category, budget.reserve(Cohort::SavePayload, 4096).unwrap())
                .unwrap();
            assert_eq!(
                *digest.value(),
                expected,
                "actual hydrated owner differs: {category:?}"
            );
        }
        assert_eq!(
            budget.retained_bytes(),
            running.bytes() + hydrated.value().cost().retained_upper_bytes
        );
        costs.push(hydrated.value().cost());
        factory_bytes.push(factory_allocated);
        let start = Instant::now();
        drop(hydrated);
        let disposal_micros = start.elapsed().as_secs_f64() * 1e6;
        times.push((hydration_micros, disposal_micros));
        stages.push(checkpoints);
        assert_eq!(budget.retained_bytes(), running.bytes());
    }
    assert_eq!(super::tests_public::simulation_stamp(world), before);
    let nav = engine
        .world()
        .nav
        .as_ref()
        .map(|n| n.checkpoint_storage_inventory());
    let config_nav_after = engine
        .config()
        .nav
        .as_ref()
        .map(|n| n.checkpoint_storage_inventory());
    assert_eq!(
        serde_json::to_value(nav_before).unwrap(),
        serde_json::to_value(nav).unwrap(),
        "capture, validation, construction and observation must not warm navigation"
    );
    assert_eq!(
        serde_json::to_value(config_nav_before).unwrap(),
        serde_json::to_value(config_nav_after).unwrap()
    );
    let admitted_asset_sums: Vec<_> = factory_bytes
        .iter()
        .map(|n| n + shared_nav_bytes + SHARED_PROMPT_RUNTIME_UPPER_BYTES)
        .collect();
    let placed = engine
        .world()
        .characters
        .values()
        .filter(|c| c.lore().is_some_and(|l| l.generated))
        .count();
    assert_eq!(
        placed, extra as usize,
        "every requested resident must actually be placed"
    );
    let report = serde_json::json!({
        "schema":"m2b-hydration-v1", "scenario":"actual-city-rich-ordinary-boundary",
        "profile":if extra==0 {"authored"} else {"populated"},
        "characters":engine.world().characters.len(),"requested":extra,"placed":placed,"unplaced":extra as usize-placed,
        "host_image":engine.config().checkpoint_host_image,
        "samples":samples,"extra":extra,"times_us":times,"stages_us":stages,
        "costs":costs,"capture_costs":capture_costs,
        "shared_peak_bytes":budget.peak_retained_bytes(),"asset_lease_bytes":ASSETS_UPPER_BYTES,
        "factory_cumulative_requested_bytes":factory_bytes,
        "retained_distinct_shared_nav_upper_bytes":shared_nav_bytes,
        "shared_prompt_runtime_upper_bytes":SHARED_PROMPT_RUNTIME_UPPER_BYTES,
        "factory_plus_retained_shared_assets_upper_bytes":admitted_asset_sums,
        "factory_counter_scope":"synchronous factory, cumulative alloc/alloc_zeroed/realloc requested bytes, no construction threads",
        "factory_non_nav_roles":"deep-cloned owned definitions; independent config/world shelters",
        "world_nav_before":nav_before,"world_nav_after":nav,
        "engine_nav_before":config_nav_before,"engine_nav_after":config_nav_after,
        "world_engine_share_graph_arc":nav_shared,
        "pending_counts":pending_counts,"pending_case":pending_case,
        "hydrated_protected_speech_action_counts":protected_speech_counts,
        "actual_owner_categories":16,"capture_boundary_unchanged":true
    });
    world.resource_mut::<Probe>().report = Some(report);
}
fn run(extra: u32, samples: usize) -> serde_json::Value {
    let mut app = super::tests::fixture(true, extra);
    app.add_systems(PostUpdate, inspect.in_set(HostCaptureSet));
    super::tests::prepare_readable_workload(&mut app);
    app.insert_resource(Probe {
        extra,
        samples,
        pending_case: 0,
        report: None,
    });
    app.update();
    app.world_mut()
        .resource_mut::<Probe>()
        .report
        .take()
        .unwrap()
}
#[test]
fn hydration_actual_authored_owners_roundtrip_without_poll_or_creation() {
    println!("hydration authored {}", run(0, 1));
}
#[test]
fn hydration_actual_populated_owners_roundtrip_without_poll_or_creation() {
    println!("hydration populated {}", run(2000, 1));
}
#[test]
#[ignore = "serial actual authored/populated hydration cost probe"]
fn m2b_hydration_probe() {
    let extra = match std::env::var("ALIBI_HYDRATION_MODE")
        .as_deref()
        .unwrap_or("authored")
    {
        "authored" => 0,
        "populated" => 2000,
        _ => panic!("invalid mode"),
    };
    let samples = std::env::var("ALIBI_HYDRATION_SAMPLES")
        .unwrap_or("1".into())
        .parse()
        .unwrap();
    assert!((1..=1000).contains(&samples));
    let report = run(extra, samples);
    let path = std::env::var("ALIBI_HYDRATION_REPORT").unwrap();
    std::fs::write(path, serde_json::to_vec_pretty(&report).unwrap()).unwrap();
}

#[test]
fn hydration_pending_scheduler_held_and_interrupted_speech_are_retained() {
    let mut app = super::tests::fixture_with_config(false, 0, |config| {
        config.idle_cognition.mode = "all".into();
    });
    app.add_systems(PostUpdate, inspect.in_set(HostCaptureSet));
    app.world()
        .resource::<smart_actors::bridge::BridgeHandle>()
        .try_send(smart_actors::bridge::BridgeCommand::PlayerAudioBegin {
            wav_basename: "m2b-interrupted.wav".into(),
        })
        .unwrap();
    app.insert_resource(Probe {
        extra: 0,
        samples: 1,
        pending_case: 1,
        report: None,
    });
    app.update();
    let report = app
        .world_mut()
        .resource_mut::<Probe>()
        .report
        .take()
        .unwrap();
    println!("hydration held/speech {report}");
}
#[test]
fn hydration_pending_night_inputs_and_duties_are_retained() {
    let mut app = super::tests::fixture_with_config(false, 0, |config| {
        config.idle_cognition.mode = "all".into();
        config.clock.start_office = "waning".into();
        config.clock.seconds_per_day = 1.0;
    });
    app.add_systems(PostUpdate, inspect.in_set(HostCaptureSet));
    app.insert_resource(Probe {
        extra: 0,
        samples: 1,
        pending_case: 2,
        report: None,
    });
    for _ in 0..120 {
        app.update();
        if let Some(report) = app.world_mut().resource_mut::<Probe>().report.take() {
            println!("hydration Night {report}");
            return;
        }
    }
    panic!("ordinary Night never submitted its defined obligation");
}

#[test]
fn hydration_accepted_recording_retains_the_actual_protected_root_index() {
    let mut app = super::tests::fixture(false, 0);
    app.add_systems(PostUpdate, inspect.in_set(HostCaptureSet));
    // Use the ordinary input producer's current body/sequence, which may be
    // newer than the previously accepted Engine sample still awaiting input.
    let world = app.world_mut();
    let position = world
        .query_filtered::<&PhysicalPosition, With<PlayerController>>()
        .single(world)
        .unwrap()
        .current;
    let spatial_seq = world
        .resource_mut::<PlayerSpatialState>()
        .position_for_action(position);
    app.world()
        .resource::<smart_actors::bridge::BridgeHandle>()
        .try_send(smart_actors::bridge::BridgeCommand::PlayerRecording {
            request_id: "m2b-accepted-recording".into(),
            wav_basename: "m2b-accepted.wav".into(),
            stt_backend: smart_actors::bridge::TranscriptionBackend::Cloud,
            position_m: smart_actors::model::Position::new(position.x, position.y, position.z)
                .unwrap(),
            spatial_seq,
        })
        .unwrap();
    app.insert_resource(Probe {
        extra: 0,
        samples: 1,
        pending_case: 3,
        report: None,
    });
    app.update();
    let report = app
        .world_mut()
        .resource_mut::<Probe>()
        .report
        .take()
        .unwrap();
    println!("hydration accepted recording {report}");
}

#[derive(Resource)]
struct FixtureRequest {
    path: PathBuf,
    write: bool,
    incompatible: bool,
    checked: bool,
    prepared: Option<FixturePrepared>,
}
struct FixturePrepared {
    candidate: checkpoint::Admitted<complete::CompleteCheckpointCandidate>,
    budget: CheckpointBudget,
    running: checkpoint::Reservation,
    definitions: checkpoint::host::DefinitionsV1,
    expected: [[u8; 32]; 16],
    raw_digest: [u8; 32],
}
fn inspect_fixture(world: &mut World) {
    use sha2::{Digest, Sha256};
    use std::io::{Read, Write};
    let Some(request) = world.get_resource::<FixtureRequest>() else {
        return;
    };
    if request.checked {
        return;
    }
    let (path, write, incompatible) = (request.path.clone(), request.write, request.incompatible);
    let stamp = super::tests_public::simulation_stamp(world);
    let budget = CheckpointBudget::default();
    let running = budget
        .reserve(Cohort::Running, complete::RUNNING_AUTHORITY_ALLOWANCE_BYTES)
        .unwrap();
    let input = if write {
        let saved = capture_complete(
            world,
            CheckpointProfile::Authored,
            budget.reserve(Cohort::SavePayload, 4096).unwrap(),
        )
        .unwrap();
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .unwrap();
        file.write_all(saved.value().bytes()).unwrap();
        CompleteCheckpointInput::copy_from(
            saved.value().bytes(),
            budget.reserve(Cohort::LoadCandidate, 4096).unwrap(),
        )
        .unwrap()
    } else {
        let mut file = std::fs::File::open(&path).unwrap();
        let len = usize::try_from(file.metadata().unwrap().len()).unwrap();
        assert!(len <= checkpoint::AUTHORED_PAYLOAD_BYTES);
        // Reserve before allocating or reading any fixture bytes. The fixed
        // exact-length read refuses append/truncation without capacity growth.
        let reservation = budget.reserve(Cohort::LoadCandidate, len + 4096).unwrap();
        let mut bytes = vec![0; len];
        file.read_exact(&mut bytes).unwrap();
        assert_eq!(file.read(&mut [0]).unwrap(), 0);
        CompleteCheckpointInput::from_owned(bytes, reservation).unwrap()
    };
    let validated = validate_complete(world, input);
    let prepared = if incompatible {
        let error = match validated {
            Ok(_) => panic!("different-image fixture unexpectedly accepted"),
            Err(e) => e,
        };
        assert_eq!(error.reason, "exact running host image mismatch");
        assert_eq!(budget.retained_bytes(), running.bytes());
        println!("hydration fixture rejected before construction: {error}");
        None
    } else {
        let candidate = validated.unwrap();
        let expected = CATEGORIES.map(|c| Sha256::digest(candidate.value().category(c)).into());
        let raw_digest = Sha256::digest(candidate.value().bytes()).into();
        Some(FixturePrepared {
            candidate,
            budget,
            running,
            definitions: HostObservation::new(world).unwrap().definitions().unwrap(),
            expected,
            raw_digest,
        })
    };
    assert_eq!(super::tests_public::simulation_stamp(world), stamp);
    let mut request = world.resource_mut::<FixtureRequest>();
    request.checked = true;
    request.prepared = prepared;
}
fn fixture_roundtrip(write: bool) {
    let path = PathBuf::from(std::env::var("ALIBI_HYDRATION_FIXTURE").unwrap());
    assert!(
        path.starts_with("/tmp"),
        "explicit temporary fixture path required"
    );
    if write {
        assert!(
            std::env::var("ALIBI_COMPLETE_WORLD_ID").is_ok(),
            "fixture lineage must be set before ordinary startup"
        );
    }
    let incompatible = std::env::var("ALIBI_HYDRATION_EXPECT_INCOMPATIBLE").is_ok();
    assert!(!write || !incompatible);
    let active = std::env::var("ALIBI_HYDRATION_INITIAL").is_err();
    let mut app = super::tests::fixture(true, 0);
    if active {
        super::tests::prepare_readable_workload(&mut app);
    }
    app.add_systems(PostUpdate, inspect_fixture.in_set(HostCaptureSet));
    app.insert_resource(FixtureRequest {
        path,
        write,
        incompatible,
        checked: false,
        prepared: None,
    });
    app.update();
    let request = app.world_mut().remove_resource::<FixtureRequest>().unwrap();
    assert!(request.checked);
    let Some(FixturePrepared {
        candidate,
        budget,
        running,
        definitions,
        expected,
        raw_digest,
    }) = request.prepared
    else {
        assert!(incompatible);
        return;
    };
    let capture_cost = candidate.value().cost();
    let boundary = candidate.value().boundary();
    let lineage = candidate.value().world_identity();
    let engine = app
        .world()
        .non_send::<smart_actors::local_engine::LocalEngine>()
        .checkpoint_engine()
        .unwrap();
    let shared_nav_bytes = retained_shared_nav_bytes(engine);
    let host_image = engine.config().checkpoint_host_image;
    let mut factory_bytes = 0;
    let hydrated = candidate
        .prepare_hydration(ASSETS_UPPER_BYTES)
        .unwrap()
        .hydrate(
            || {
                let (assets, bytes) = factory_allocations(|| {
                    let engine = app
                        .world()
                        .non_send::<smart_actors::local_engine::LocalEngine>()
                        .checkpoint_engine()
                        .unwrap();
                    let assets = installed_assets(engine, 0);
                    drop(app);
                    assets
                });
                factory_bytes = bytes;
                assets
            },
            definitions,
            RuntimeGeneration(u64::MAX),
        )
        .unwrap();
    assert!(
        factory_bytes + shared_nav_bytes + SHARED_PROMPT_RUNTIME_UPPER_BYTES <= ASSETS_UPPER_BYTES
    );
    assert_eq!(hydrated.value().boundary(), boundary);
    assert_eq!(hydrated.value().world_identity(), lineage);
    for (category, expected) in CATEGORIES.into_iter().zip(expected) {
        assert_eq!(
            *hydrated
                .value()
                .category_digest(category, budget.reserve(Cohort::SavePayload, 4096).unwrap())
                .unwrap()
                .value(),
            expected
        );
    }
    let report = serde_json::json!({
        "schema":"m2b-hydration-fixture-v1", "writer":write, "active":active,
        "host_image":host_image, "raw_sha256":raw_digest,"category_sha256":expected,
        "lineage":lineage.bytes(), "boundary":boundary.seconds(),
        "capture_cost":capture_cost,"hydration_cost":hydrated.value().cost(),
        "factory_cumulative_requested_bytes":factory_bytes,
        "retained_distinct_shared_nav_upper_bytes":shared_nav_bytes,
        "shared_prompt_runtime_upper_bytes":SHARED_PROMPT_RUNTIME_UPPER_BYTES,
        "factory_plus_retained_shared_assets_upper_bytes":factory_bytes+shared_nav_bytes+SHARED_PROMPT_RUNTIME_UPPER_BYTES,
        "shared_peak_bytes":budget.peak_retained_bytes(),"actual_owner_categories":16,
        "source_app_disposed_before_fresh_resolver":true
    });
    drop(hydrated);
    assert_eq!(budget.retained_bytes(), running.bytes());
    if let Ok(path) = std::env::var("ALIBI_HYDRATION_REPORT") {
        std::fs::write(path, serde_json::to_vec_pretty(&report).unwrap()).unwrap();
    }
    println!("hydration fixture {report}");
}
#[test]
#[ignore = "explicit same-image temporary fixture writer"]
fn m2b_hydration_fixture_write() {
    fixture_roundtrip(true);
}
#[test]
#[ignore = "explicit fresh-process temporary fixture reader"]
fn m2b_hydration_fixture_read() {
    fixture_roundtrip(false);
}
