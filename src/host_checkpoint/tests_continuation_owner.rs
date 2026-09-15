//! Actual authored/populated continuation preparation and same-image V2 probe.
use super::tests_hydration_owner::{ASSETS_UPPER_BYTES, CATEGORIES, installed_assets};
use super::*;
use cathedral_sim::{
    Capabilities, Cognition, CognitionBusy, NullSight, NullTranscription, NullTts, RequestId,
    RuntimeGeneration,
    checkpoint::{
        CheckpointBudget, Cohort,
        complete::{
            self, CheckpointCategory as C, CheckpointProfile, CompleteCheckpointInput,
            ContinuationServices,
        },
    },
};
use std::{path::PathBuf, time::Instant};

const SERVICES: usize = 64 * 1024;
struct InertCognition;
impl Cognition for InertCognition {
    fn request(&mut self, _: String) -> std::result::Result<RequestId, CognitionBusy> {
        panic!("quarantined probe submitted cognition")
    }
}
#[derive(Resource)]
struct Probe {
    extra: u32,
    samples: usize,
    report: Option<serde_json::Value>,
    fixture_path: Option<PathBuf>,
    read_fixture: bool,
}

fn inspect(world: &mut World) {
    let Some(probe) = world.get_resource::<Probe>() else {
        return;
    };
    if probe.report.is_some() {
        return;
    }
    let (extra, samples) = (probe.extra, probe.samples);
    let (fixture_path, read_fixture) = (probe.fixture_path.clone(), probe.read_fixture);
    let before = super::tests_public::simulation_stamp(world);
    let source = HostObservation::new(world).unwrap();
    let definitions = source.definitions().unwrap();
    let engine = source.local().unwrap().checkpoint_engine().unwrap();
    let generation = engine.config().runtime_generation.successor().unwrap();
    let profile = if extra == 0 {
        CheckpointProfile::Authored
    } else {
        CheckpointProfile::Populated
    };
    let budget = CheckpointBudget::default();
    let running = budget
        .reserve(Cohort::Running, complete::RUNNING_AUTHORITY_ALLOWANCE_BYTES)
        .unwrap();
    let mut times = Vec::with_capacity(samples);
    let mut stages = Vec::with_capacity(samples);
    let mut reports = Vec::with_capacity(samples);
    let mut capture_costs = Vec::with_capacity(samples);
    let mut pending_shapes = Vec::with_capacity(samples);
    let mut input_hashes: Vec<[u8; 32]> = Vec::with_capacity(samples);
    let mut fixture_hash = None;
    let mut resave_hash = None;
    let mut second_save_bytes = 0;
    for sample in 0..samples {
        let saved = if read_fixture {
            use std::io::Read;
            let mut file = std::fs::File::open(fixture_path.as_ref().unwrap()).unwrap();
            let len = usize::try_from(file.metadata().unwrap().len()).unwrap();
            assert!(len <= 128 * 1024 * 1024);
            let reservation = budget.reserve(Cohort::LoadCandidate, len + 4096).unwrap();
            let mut bytes = vec![0; len];
            file.read_exact(&mut bytes).unwrap();
            assert_eq!(file.read(&mut [0]).unwrap(), 0);
            validate_complete(
                world,
                CompleteCheckpointInput::from_owned(bytes, reservation).unwrap(),
            )
            .unwrap()
        } else {
            capture_complete(
                world,
                profile,
                budget.reserve(Cohort::SavePayload, 4096).unwrap(),
            )
            .unwrap()
        };
        let saved_speech: serde_json::Value =
            serde_json::from_slice(saved.value().category(C::Speech)).unwrap();
        let speech = if saved_speech["version"] == 2 {
            &saved_speech["base"]["state"]
        } else {
            &saved_speech["state"]
        };
        let inputs: serde_json::Value =
            serde_json::from_slice(saved.value().category(C::CognitionInputs)).unwrap();
        let scheduler: serde_json::Value =
            serde_json::from_slice(saved.value().category(C::Scheduler)).unwrap();
        let night: serde_json::Value =
            serde_json::from_slice(saved.value().category(C::Night)).unwrap();
        let scheduler_base = if scheduler["version"] == 2 {
            &scheduler["base"]["scheduler"]
        } else {
            &scheduler["scheduler"]
        };
        let night_base = if night["version"] == 2 {
            &night["base"]["night"]["night"]
        } else {
            &night["night"]["night"]
        };
        if !read_fixture {
            assert_eq!(
                speech["accepted_recordings"].as_array().unwrap().len(),
                1,
                "ordinary accepted recording required"
            );
            assert!(
                !inputs["scheduler"].is_null(),
                "ordinary submitted cognition required"
            );
        }
        pending_shapes.push(serde_json::json!({"scheduler_submitted":!scheduler_base["in_flight"].is_null(),
            "scheduler_held":!scheduler_base["held_result"].is_null(),
            "scheduler_unfinished":!scheduler_base["in_flight"].is_null()&&scheduler_base["held_result"].is_null(),
            "scheduler_deferred":scheduler["continuation"]["load_retries"].as_array().map_or(0,Vec::len),
            "night_submitted":!night_base["in_flight"].is_null(),"night_held":!night_base["held_result"].is_null(),
            "speech_captures":speech["captures"].as_array().unwrap().len(),"speech_streams":speech["streams"].as_array().unwrap().len(),
            "accepted_recordings":speech["accepted_recordings"].as_array().unwrap().len()}));
        use sha2::{Digest, Sha256};
        input_hashes.push(Sha256::digest(saved.value().bytes()).into());
        let original_digests: Vec<_> = CATEGORIES
            .iter()
            .map(|c| {
                (
                    *c,
                    <[u8; 32]>::from(Sha256::digest(saved.value().category(*c))),
                )
            })
            .collect();
        capture_costs.push(saved.value().cost());
        let candidate = if read_fixture {
            saved
        } else {
            let candidate = validate_complete(
                world,
                CompleteCheckpointInput::copy_from(
                    saved.value().bytes(),
                    budget.reserve(Cohort::LoadCandidate, 4096).unwrap(),
                )
                .unwrap(),
            )
            .unwrap();
            drop(saved);
            candidate
        };
        let hydrated = candidate
            .prepare_hydration(ASSETS_UPPER_BYTES)
            .unwrap()
            .hydrate(|| installed_assets(engine, extra), definitions, generation)
            .unwrap();
        let t = Instant::now();
        let mut previous = t;
        let mut stage_times = vec![];
        let prepared = hydrated
            .prepare_continuation_observed(&mut |stage| {
                let next = Instant::now();
                stage_times.push((stage, next.duration_since(previous).as_secs_f64() * 1e6));
                previous = next;
            })
            .unwrap();
        let prepare_us = t.elapsed().as_secs_f64() * 1e6;
        let report = prepared.value().report();
        assert_eq!(report.interruption_receipts, 1);
        assert!(report.scheduler_retries == 1 || report.scheduler_held);
        for (c, expected) in &original_digests {
            if matches!(
                c,
                C::Backbone
                    | C::Round
                    | C::Climate
                    | C::Law
                    | C::Marks
                    | C::Animals
                    | C::Social
                    | C::Host
            ) {
                assert_eq!(
                    *prepared
                        .value()
                        .category_digest(*c, budget.reserve(Cohort::SavePayload, 4096).unwrap())
                        .unwrap()
                        .value(),
                    *expected
                );
            }
        }
        let t = Instant::now();
        let bound = prepared
            .bind_services(SERVICES, |generation| {
                Ok(ContinuationServices {
                    generation,
                    cognition: Box::new(InertCognition),
                    transcription: Box::new(NullTranscription),
                    tts: Box::new(NullTts),
                    sight: Box::new(NullSight),
                    capabilities: Capabilities::default(),
                    runtime_dir: PathBuf::from("/tmp/m2c-inert-services"),
                })
            })
            .unwrap();
        let bind_us = t.elapsed().as_secs_f64() * 1e6;
        assert!(bound.value().report().services_bound);
        reports.push(bound.value().report());
        let retained_before_drop = budget.retained_bytes();
        assert_eq!(
            retained_before_drop,
            running.bytes() + ASSETS_UPPER_BYTES + SERVICES + report.typed_upper_bytes
        );
        if sample == 0 {
            let saved = bound
                .value()
                .capture(profile, budget.reserve(Cohort::SavePayload, 4096).unwrap())
                .unwrap();
            second_save_bytes = saved.value().bytes().len();
            // Admit the encoded copy only after disposing the original
            // prepared generation; never retain an uncharged raw copy.
            let expected: [u8; 32] = Sha256::digest(saved.value().bytes()).into();
            if read_fixture {
                assert_eq!(
                    expected, input_hashes[0],
                    "prepared V2 reader must preserve original input fixture bytes"
                );
            }
            fixture_hash = Some(expected);
            if !read_fixture && let Some(path) = &fixture_path {
                use std::io::Write;
                let mut file = std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(path)
                    .unwrap();
                file.write_all(saved.value().bytes()).unwrap();
            }
            let t = Instant::now();
            drop(bound);
            let disposal_us = t.elapsed().as_secs_f64() * 1e6;
            let input = CompleteCheckpointInput::copy_from(
                saved.value().bytes(),
                budget.reserve(Cohort::LoadCandidate, 4096).unwrap(),
            )
            .unwrap();
            drop(saved);
            let repeated = validate_complete(world, input).unwrap();
            let again = repeated
                .prepare_hydration(ASSETS_UPPER_BYTES)
                .unwrap()
                .hydrate(
                    || installed_assets(engine, extra),
                    definitions,
                    RuntimeGeneration(generation.0 + 1),
                )
                .unwrap()
                .prepare_continuation()
                .unwrap();
            let repeated = again
                .value()
                .capture(profile, budget.reserve(Cohort::SavePayload, 4096).unwrap())
                .unwrap();
            assert_eq!(
                <[u8; 32]>::from(Sha256::digest(repeated.value().bytes())),
                expected
            );
            resave_hash = Some(<[u8; 32]>::from(Sha256::digest(repeated.value().bytes())));
            drop(repeated);
            drop(again);
            times.push((prepare_us, bind_us, disposal_us));
        } else {
            let t = Instant::now();
            drop(bound);
            times.push((prepare_us, bind_us, t.elapsed().as_secs_f64() * 1e6));
        }
        stages.push(stage_times);
        assert_eq!(budget.retained_bytes(), running.bytes());
    }
    assert_eq!(super::tests_public::simulation_stamp(world), before);
    let placed = engine
        .world()
        .characters
        .values()
        .filter(|c| c.lore().is_some_and(|l| l.generated))
        .count();
    assert_eq!(placed, extra as usize);
    let report = serde_json::json!({"schema":"m2c-continuation-v1","profile":if extra==0{"authored"}else{"populated"},
        "scenario":"actual-city-pending-cognition-and-accepted-recording", "characters":engine.world().characters.len(),
        "requested":extra,"placed":placed,"unplaced":0,"samples":samples,"host_image":engine.config().checkpoint_host_image,
        "times_us":times,"time_columns":["preparation","service_binding","prepared_owner_disposal"],"stages_us":stages,
        "reports":reports,"capture_costs":capture_costs,"shared_peak_bytes":budget.peak_retained_bytes(),
        "asset_lease_bytes":ASSETS_UPPER_BYTES,"service_lease_bytes":SERVICES,"immediate_v2_resave_bytes":second_save_bytes,
        "pending_shapes":pending_shapes,"fixture_reader":read_fixture,"unchanged_category_count":8,"unchanged_categories_equal":true,
        "input_sha256":input_hashes,"fixture_sha256":fixture_hash,"resave_sha256":resave_hash,
        "second_preparation_exact_bytes":true,"capture_boundary_unchanged":true,
        "scope":"quarantined prepared owners and inert service handles; excludes host retirement, device construction and M3 adoption"});
    world.resource_mut::<Probe>().report = Some(report);
}

fn run(extra: u32, samples: usize) -> serde_json::Value {
    let mut app = super::tests::fixture_with_config(true, extra, |_| {});
    super::tests::prepare_readable_workload(&mut app);
    let world = app.world_mut();
    let position = world
        .query_filtered::<&PhysicalPosition, With<PlayerController>>()
        .single(world)
        .unwrap()
        .current;
    let spatial_seq = world
        .resource_mut::<PlayerSpatialState>()
        .position_for_action(position);
    let bridge = world.resource::<smart_actors::bridge::BridgeHandle>();
    bridge
        .try_send(smart_actors::bridge::BridgeCommand::PlayerAudioBegin {
            wav_basename: "m2c-probe.wav".into(),
        })
        .unwrap();
    bridge
        .try_send(smart_actors::bridge::BridgeCommand::PlayerRecording {
            request_id: "m2c-probe-recording".into(),
            wav_basename: "m2c-probe.wav".into(),
            stt_backend: smart_actors::bridge::TranscriptionBackend::Cloud,
            position_m: smart_actors::model::Position::new(position.x, position.y, position.z)
                .unwrap(),
            spatial_seq,
        })
        .unwrap();
    app.add_systems(PostUpdate, inspect.in_set(HostCaptureSet));
    let fixture_path = std::env::var("ALIBI_CONTINUATION_FIXTURE")
        .ok()
        .map(PathBuf::from);
    if let Some(path) = &fixture_path {
        assert!(path.starts_with("/tmp"));
    }
    let read_fixture = std::env::var("ALIBI_CONTINUATION_READ_FIXTURE").is_ok();
    assert!(!read_fixture || fixture_path.is_some());
    app.insert_resource(Probe {
        extra,
        samples,
        report: None,
        fixture_path,
        read_fixture,
    });
    app.update();
    app.world_mut()
        .resource_mut::<Probe>()
        .report
        .take()
        .unwrap()
}
#[test]
#[ignore = "serial actual authored/populated pending continuation probe"]
fn m2c_continuation_probe() {
    let extra = match std::env::var("ALIBI_CONTINUATION_MODE")
        .as_deref()
        .unwrap_or("authored")
    {
        "authored" => 0,
        "populated" => 2000,
        _ => panic!("invalid continuation mode"),
    };
    let samples: usize = std::env::var("ALIBI_CONTINUATION_SAMPLES")
        .unwrap_or("1".into())
        .parse()
        .unwrap();
    assert!((1..=1000).contains(&samples));
    let report = run(extra, samples);
    std::fs::write(
        std::env::var("ALIBI_CONTINUATION_REPORT").unwrap(),
        serde_json::to_vec_pretty(&report).unwrap(),
    )
    .unwrap();
}
