//! Coordinator-owned assertions through the public quarantined continuation API.
//! Shared helpers install assets; preparation, admission and receipt assertions
//! are independent of the implementation owner's tests.
use super::{HostCaptureSet, HostObservation, capture_complete, validate_complete};
use bevy::prelude::*;
use cathedral_sim::{
    Capabilities, Cognition, CognitionBusy, RequestId, RuntimeGeneration, Sight, SttBackendKind,
    SttSubmitError, Transcription, TranscriptionJobId, Tts, TtsBackendKind, TtsRequest,
    TtsSubmitError,
    checkpoint::{
        Admitted, CheckpointBudget, CheckpointError, Cohort, MAX_RESIDENT_BYTES, Reservation,
        complete::{
            CheckpointCategory as Category, CheckpointProfile, CompleteCheckpointCandidate,
            CompleteCheckpointInput, ContinuationServices, ContinuationStage, HydratedEngine,
            PreparedContinuation, RUNNING_AUTHORITY_ALLOWANCE_BYTES,
        },
        host::DefinitionsV1,
    },
};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
    path::PathBuf,
    rc::Rc,
    sync::Arc,
};

const ASSETS: usize = 64 * 1024 * 1024;
const SERVICES: usize = 64 * 1024;

#[derive(Resource)]
struct Request(Option<Captured>);

struct Captured {
    candidate: Admitted<CompleteCheckpointCandidate>,
    budget: Arc<CheckpointBudget>,
    running: Reservation,
    definitions: DefinitionsV1,
    raw: Value,
    generation: RuntimeGeneration,
    digests: Vec<(Category, [u8; 32])>,
}

fn inspect(world: &mut World) {
    if !world.contains_resource::<Request>() {
        return;
    }
    let before = super::tests_public::simulation_stamp(world);
    let budget = Arc::new(CheckpointBudget::default());
    let running = budget
        .reserve(Cohort::Running, RUNNING_AUTHORITY_ALLOWANCE_BYTES)
        .unwrap();
    let saved = capture_complete(
        world,
        CheckpointProfile::Authored,
        budget.reserve(Cohort::SavePayload, 4096).unwrap(),
    )
    .unwrap();
    let raw = serde_json::from_slice(saved.value().bytes()).unwrap();
    let digests = super::tests_hydration_owner::CATEGORIES
        .into_iter()
        .map(|c| (c, Sha256::digest(saved.value().category(c)).into()))
        .collect();
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
    let source = HostObservation::new(world).unwrap();
    let generation = source
        .local()
        .unwrap()
        .checkpoint_engine()
        .unwrap()
        .config()
        .runtime_generation;
    let captured = Captured {
        candidate,
        budget,
        running,
        definitions: source.definitions().unwrap(),
        raw,
        generation,
        digests,
    };
    assert_eq!(super::tests_public::simulation_stamp(world), before);
    world.resource_mut::<Request>().0 = Some(captured);
}

fn fixture(recording: bool) -> (App, Captured) {
    let mut app = super::tests_public::app();
    if recording {
        use crate::smart_actors::{
            bridge::{BridgeCommand, BridgeHandle, TranscriptionBackend},
            interaction::PlayerSpatialState,
            model::Position,
        };
        let world = app.world_mut();
        let position = world
            .query_filtered::<&crate::controller::PhysicalPosition, With<crate::controller::PlayerController>>()
            .single(world)
            .unwrap()
            .current;
        let spatial_seq = world
            .resource_mut::<PlayerSpatialState>()
            .position_for_action(position);
        world
            .resource::<BridgeHandle>()
            .try_send(BridgeCommand::PlayerRecording {
                request_id: "m2c-public-recording".into(),
                wav_basename: "m2c-public-recording.wav".into(),
                stt_backend: TranscriptionBackend::Cloud,
                position_m: Position::new(position.x, position.y, position.z).unwrap(),
                spatial_seq,
            })
            .unwrap();
    }
    app.add_systems(PostUpdate, inspect.in_set(HostCaptureSet));
    app.insert_resource(Request(None));
    app.update();
    let captured = app
        .world_mut()
        .remove_resource::<Request>()
        .unwrap()
        .0
        .unwrap();
    (app, captured)
}

fn hydrate(
    app: &App,
    captured: Captured,
) -> (
    Admitted<HydratedEngine>,
    Arc<CheckpointBudget>,
    Reservation,
    Value,
    Vec<(Category, [u8; 32])>,
) {
    let Captured {
        candidate,
        budget,
        running,
        definitions,
        raw,
        generation,
        digests,
    } = captured;
    let engine = app
        .world()
        .non_send::<crate::smart_actors::local_engine::LocalEngine>()
        .checkpoint_engine()
        .unwrap();
    let hydrated = candidate
        .prepare_hydration(ASSETS)
        .unwrap()
        .hydrate(
            || super::tests_hydration_owner::installed_assets(engine, 0),
            definitions,
            generation.successor().unwrap(),
        )
        .unwrap();
    (hydrated, budget, running, raw, digests)
}

struct InertService {
    budget: Arc<CheckpointBudget>,
    drops: Rc<Cell<usize>>,
    minimum_charge: usize,
}
impl Drop for InertService {
    fn drop(&mut self) {
        assert!(
            self.budget.retained_bytes() >= self.minimum_charge,
            "service outlived its admitted storage"
        );
        self.drops.set(self.drops.get() + 1);
    }
}
impl Cognition for InertService {
    fn request(&mut self, _: String) -> Result<RequestId, CognitionBusy> {
        panic!("binding submitted cognition")
    }
    fn request_with_budget(
        &mut self,
        _: String,
        _: Option<u32>,
    ) -> Result<RequestId, CognitionBusy> {
        panic!("binding submitted budgeted cognition")
    }
    fn request_night(&mut self, _: String, _: Option<u32>) -> Result<RequestId, CognitionBusy> {
        panic!("binding submitted Night cognition")
    }
}
impl Transcription for InertService {
    fn available(&self, _: SttBackendKind) -> bool {
        panic!("binding probed STT")
    }
    fn submit_batch(
        &mut self,
        _: TranscriptionJobId,
        _: PathBuf,
        _: SttBackendKind,
    ) -> Result<(), SttSubmitError> {
        panic!("binding submitted STT")
    }
    fn realtime_begin(&mut self, _: &str) -> bool {
        panic!("binding began STT")
    }
    fn realtime_append(&mut self, _: &str, _: &[i16]) -> bool {
        panic!("binding appended PCM")
    }
    fn realtime_commit(&mut self, _: &str) -> bool {
        panic!("binding committed STT")
    }
    fn realtime_clear(&mut self, _: &str) {
        panic!("binding cleared STT")
    }
    fn recording_seconds(&self, _: &std::path::Path) -> Option<f64> {
        panic!("binding read recording")
    }
    fn discard_recording(&mut self, _: &std::path::Path) {
        panic!("binding discarded recording")
    }
}
impl Tts for InertService {
    fn available(&self, _: TtsBackendKind) -> bool {
        panic!("binding probed TTS")
    }
    fn submit(&mut self, _: TtsRequest) -> Result<(), TtsSubmitError> {
        panic!("binding submitted TTS")
    }
    fn warm(&mut self, _: TtsBackendKind) {
        panic!("binding warmed TTS")
    }
}
impl Sight for InertService {
    fn line_of_sight(&self, _: cathedral_sim::Vec3, _: cathedral_sim::Vec3) -> bool {
        panic!("binding queried sight")
    }
    fn npc_pov_frame(&mut self, _: &cathedral_sim::ActorId) -> Option<cathedral_sim::PovFrame> {
        panic!("binding requested a frame")
    }
}

fn services(
    generation: RuntimeGeneration,
    budget: &Arc<CheckpointBudget>,
    drops: &Rc<Cell<usize>>,
    minimum_charge: usize,
) -> ContinuationServices {
    let inert = || InertService {
        budget: Arc::clone(budget),
        drops: Rc::clone(drops),
        minimum_charge,
    };
    ContinuationServices {
        generation,
        cognition: Box::new(inert()),
        transcription: Box::new(inert()),
        tts: Box::new(inert()),
        sight: Box::new(inert()),
        capabilities: Capabilities::default(),
        runtime_dir: PathBuf::from("/tmp/m2c-public-inert-service-owner"),
    }
}

fn digest(value: &PreparedContinuation, category: Category, budget: &CheckpointBudget) -> [u8; 32] {
    *value
        .category_digest(category, budget.reserve(Cohort::SavePayload, 4096).unwrap())
        .unwrap()
        .value()
}

#[test]
fn preparation_preserves_saved_host_time_lineage_and_unrelated_actual_owners() {
    let (app, captured) = fixture(false);
    let (hydrated, budget, running, raw, digests) = hydrate(&app, captured);
    let boundary = hydrated.value().boundary();
    let lineage = hydrated.value().world_identity();
    let generation = hydrated.value().runtime_generation();
    let old_generation = hydrated.value().host().scalars().boundary.generation;
    let before = super::tests_public::simulation_stamp(app.world());
    let mut stages = Vec::new();
    let prepared = hydrated
        .prepare_continuation_observed(&mut |stage| stages.push(stage))
        .unwrap();
    assert!(matches!(
        stages.as_slice(),
        [
            ContinuationStage::Admission,
            ContinuationStage::Inputs,
            ContinuationStage::Speech,
            ContinuationStage::Floor,
            ContinuationStage::Prepared
        ]
    ));
    assert_eq!(super::tests_public::simulation_stamp(app.world()), before);
    drop(app);
    let value = prepared.value();
    let report = value.report();
    assert_eq!(value.boundary(), boundary);
    assert_eq!(value.world_identity(), lineage);
    assert_eq!(value.runtime_generation(), generation);
    assert_eq!(value.host().scalars().boundary.generation, old_generation);
    assert_ne!(generation.0, old_generation);
    assert!(!report.services_bound);
    let flight = !raw["scheduler"]["scheduler"]["in_flight"].is_null();
    let held = !raw["scheduler"]["scheduler"]["held_result"].is_null();
    assert_eq!(report.scheduler_retries, usize::from(flight && !held));
    assert_eq!(report.scheduler_held, held);
    assert!(!report.scheduler_resumed);
    assert_eq!(report.interrupted_inputs, 0);
    assert_eq!(report.interruption_receipts, 0);
    for (category, expected) in digests {
        if matches!(
            category,
            Category::Operations
                | Category::Backbone
                | Category::Round
                | Category::Climate
                | Category::Law
                | Category::Marks
                | Category::Animals
                | Category::Social
                | Category::Host
        ) {
            assert_eq!(
                digest(value, category, &budget),
                expected,
                "preparation changed {category:?}"
            );
        }
    }
    assert!(report.typed_upper_bytes <= 128 * 1024 * 1024);
    assert_eq!(
        budget.retained_bytes(),
        running.bytes() + ASSETS + report.typed_upper_bytes
    );
    drop(prepared);
    assert_eq!(budget.retained_bytes(), running.bytes());
    drop(running);
    assert_eq!(budget.retained_bytes(), 0);
}

#[test]
fn preparation_refuses_shared_pressure_before_transforming_and_unwinds_each_stage() {
    let (app, captured) = fixture(false);
    let (hydrated, budget, running, _, _) = hydrate(&app, captured);
    let before = super::tests_public::simulation_stamp(app.world());
    let retiring = budget
        .reserve(
            Cohort::RetiringGeneration,
            MAX_RESIDENT_BYTES - budget.retained_bytes(),
        )
        .unwrap();
    let observed = Cell::new(false);
    let result = hydrated.prepare_continuation_observed(&mut |_| observed.set(true));
    assert!(result.is_err());
    assert!(!observed.get());
    assert_eq!(super::tests_public::simulation_stamp(app.world()), before);
    assert_eq!(budget.retained_bytes(), running.bytes() + retiring.bytes());
    drop(retiring);
    for fail_at in 0..5 {
        let (app, captured) = fixture(false);
        let (hydrated, budget, running, _, _) = hydrate(&app, captured);
        let before = super::tests_public::simulation_stamp(app.world());
        let mut index = 0;
        let result = catch_unwind(AssertUnwindSafe(|| {
            hydrated.prepare_continuation_observed(&mut |_| {
                let current = index;
                index += 1;
                assert_ne!(current, fail_at, "independent observer unwind");
            })
        }));
        assert!(result.is_err(), "observer stage {fail_at} did not run");
        assert_eq!(super::tests_public::simulation_stamp(app.world()), before);
        assert_eq!(budget.retained_bytes(), running.bytes());
    }
}

#[test]
fn admitted_service_binding_is_inert_preserves_state_and_drops_services_before_leases() {
    let (app, captured) = fixture(false);
    let (hydrated, budget, running, _, _) = hydrate(&app, captured);
    let prepared = hydrated.prepare_continuation().unwrap();
    let before: Vec<_> = super::tests_hydration_owner::CATEGORIES
        .into_iter()
        .map(|category| (category, digest(prepared.value(), category, &budget)))
        .collect();
    let expected_generation = prepared.value().runtime_generation();
    let before_charge = budget.retained_bytes();
    let drops = Rc::new(Cell::new(0));
    let called = Cell::new(0);
    let bound = prepared
        .bind_services(SERVICES, |generation| {
            called.set(called.get() + 1);
            assert_eq!(generation, expected_generation);
            assert_eq!(budget.retained_bytes(), before_charge + SERVICES);
            Ok(services(
                generation,
                &budget,
                &drops,
                before_charge + SERVICES,
            ))
        })
        .unwrap();
    assert_eq!(called.get(), 1);
    assert_eq!(drops.get(), 0);
    assert!(bound.value().report().services_bound);
    for (category, expected) in before {
        assert_eq!(
            digest(bound.value(), category, &budget),
            expected,
            "binding changed {category:?}"
        );
    }
    drop(bound);
    assert_eq!(drops.get(), 4);
    assert_eq!(budget.retained_bytes(), running.bytes());
}

#[test]
fn service_factory_refusal_mismatch_unwind_and_repeat_binding_release_all_owners() {
    for mode in 0..3 {
        let (app, captured) = fixture(false);
        let (hydrated, budget, running, _, _) = hydrate(&app, captured);
        let prepared = hydrated.prepare_continuation().unwrap();
        let minimum_charge = budget.retained_bytes() + SERVICES;
        let drops = Rc::new(Cell::new(0));
        let result = catch_unwind(AssertUnwindSafe(|| {
            prepared.bind_services(SERVICES, |generation| {
                let handles = services(
                    if mode == 0 {
                        generation.successor().unwrap()
                    } else {
                        generation
                    },
                    &budget,
                    &drops,
                    minimum_charge,
                );
                if mode == 1 {
                    drop(handles);
                    return Err(CheckpointError {
                        owner: "independent_service_fixture",
                        reason: "factory refusal".into(),
                    });
                }
                assert_ne!(mode, 2, "independent service factory unwind");
                Ok(handles)
            })
        }));
        if mode == 2 {
            assert!(result.is_err());
        } else {
            assert!(result.unwrap().is_err());
        }
        assert_eq!(drops.get(), 4);
        assert_eq!(budget.retained_bytes(), running.bytes());
    }
    let (app, captured) = fixture(false);
    let (hydrated, budget, running, _, _) = hydrate(&app, captured);
    let prepared = hydrated.prepare_continuation().unwrap();
    let minimum_charge = budget.retained_bytes() + SERVICES;
    let drops = Rc::new(Cell::new(0));
    let bound = prepared
        .bind_services(SERVICES, |generation| {
            Ok(services(generation, &budget, &drops, minimum_charge))
        })
        .unwrap();
    let called = Cell::new(false);
    assert!(
        bound
            .bind_services(SERVICES, |_| {
                called.set(true);
                panic!("second factory must not run")
            })
            .is_err()
    );
    assert!(!called.get());
    assert_eq!(drops.get(), 4);
    assert_eq!(budget.retained_bytes(), running.bytes());
}

#[test]
fn service_admission_precedes_factory_and_observation_requires_the_same_budget() {
    let (app, captured) = fixture(false);
    let (hydrated, budget, running, _, _) = hydrate(&app, captured);
    let prepared = hydrated.prepare_continuation().unwrap();
    let unrelated = CheckpointBudget::default();
    assert!(
        prepared
            .value()
            .category_digest(
                Category::Host,
                unrelated.reserve(Cohort::SavePayload, 4096).unwrap()
            )
            .is_err()
    );
    assert!(
        prepared
            .value()
            .capture(
                CheckpointProfile::Authored,
                unrelated.reserve(Cohort::SavePayload, 4096).unwrap()
            )
            .is_err()
    );
    assert_eq!(unrelated.retained_bytes(), 0);
    let retiring = budget
        .reserve(
            Cohort::RetiringGeneration,
            MAX_RESIDENT_BYTES - budget.retained_bytes(),
        )
        .unwrap();
    let called = Cell::new(false);
    assert!(
        prepared
            .bind_services(SERVICES, |_| {
                called.set(true);
                panic!("unadmitted service factory must not run")
            })
            .is_err()
    );
    assert!(!called.get());
    assert_eq!(budget.retained_bytes(), running.bytes() + retiring.bytes());
}

#[test]
fn accepted_recording_becomes_unsent_once_and_survives_an_immediate_second_save_load() {
    use cathedral_sim::receipts::ReceiptState;
    let (app, captured) = fixture(true);
    let (hydrated, budget, running, _, _) = hydrate(&app, captured);
    let recordings = hydrated.value().speech().accepted_recordings();
    assert_eq!(
        recordings.len(),
        1,
        "ordinary recording was not accepted before capture"
    );
    let recording = &recordings[0];
    assert_eq!(recording.request_id(), "m2c-public-recording");
    let id = recording.semantic().unwrap();
    let prior = recording.receipt().unwrap().clone();
    assert!(matches!(
        prior.outcome.state,
        ReceiptState::Accepted | ReceiptState::InProgress
    ));
    assert!(hydrated.value().protects_speech_action(id));
    let prepared = hydrated.prepare_continuation().unwrap();
    let value = prepared.value();
    assert!(!value.protects_operation(id.operation));
    assert_eq!(value.report().interruption_receipts, 1);
    assert_eq!(value.interrupted_speech().len(), 1);
    let interrupted = &value.interrupted_speech()[0];
    assert_eq!(interrupted.recordings()[0].semantic(), Some(id));
    assert_eq!(interrupted.recordings()[0].receipt(), Some(&prior));
    assert_eq!(interrupted.receipts().len(), 1);
    let terminal = &interrupted.receipts()[0];
    assert_eq!(terminal.id, prior.id);
    assert_eq!(terminal.ordinal, prior.ordinal);
    assert_eq!(terminal.affected, prior.affected);
    assert_eq!(terminal.outcome.state, ReceiptState::Interrupted);
    assert_eq!(terminal.outcome.code, "recording_interrupted_by_load");
    assert_eq!(terminal.at.to_bits(), value.boundary().seconds().to_bits());
    let original_interruption = serde_json::to_value(value.interrupted_speech()).unwrap();
    let next_generation = value.runtime_generation().successor().unwrap();
    let definitions = value.host().scalars().definitions;
    let saved = value
        .capture(
            CheckpointProfile::Authored,
            budget.reserve(Cohort::SavePayload, 4096).unwrap(),
        )
        .unwrap();
    let bytes = saved.value().bytes().to_vec();
    assert_eq!(
        serde_json::from_slice::<Value>(saved.value().category(Category::Speech)).unwrap()["version"],
        2
    );
    drop(saved);
    drop(prepared);
    assert_eq!(budget.retained_bytes(), running.bytes());
    let candidate = validate_complete(
        app.world(),
        CompleteCheckpointInput::copy_from(
            &bytes,
            budget.reserve(Cohort::LoadCandidate, 4096).unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    let engine = app
        .world()
        .non_send::<crate::smart_actors::local_engine::LocalEngine>()
        .checkpoint_engine()
        .unwrap();
    let reloaded = candidate
        .prepare_hydration(ASSETS)
        .unwrap()
        .hydrate(
            || super::tests_hydration_owner::installed_assets(engine, 0),
            definitions,
            next_generation,
        )
        .unwrap()
        .prepare_continuation()
        .unwrap();
    assert_eq!(
        serde_json::to_value(reloaded.value().interrupted_speech()).unwrap(),
        original_interruption
    );
    assert_eq!(reloaded.value().report().interruption_receipts, 1);
    assert!(!reloaded.value().protects_operation(id.operation));
    let repeated = reloaded
        .value()
        .capture(
            CheckpointProfile::Authored,
            budget.reserve(Cohort::SavePayload, 4096).unwrap(),
        )
        .unwrap();
    assert_eq!(repeated.value().bytes(), bytes);
    drop(repeated);
    drop(reloaded);
    assert_eq!(budget.retained_bytes(), running.bytes());
}

fn validate_wire(
    app: &App,
    budget: &CheckpointBudget,
    wire: &Value,
) -> cathedral_sim::checkpoint::Result<Admitted<CompleteCheckpointCandidate>> {
    let bytes = serde_json::to_vec(wire).unwrap();
    validate_complete(
        app.world(),
        CompleteCheckpointInput::copy_from(
            &bytes,
            budget.reserve(Cohort::LoadCandidate, 4096).unwrap(),
        )
        .unwrap(),
    )
}

#[test]
fn complete_v2_extensions_are_required_closed_and_check_interruption_provenance() {
    use serde_json::json;
    let (app, captured) = fixture(true);
    let (hydrated, budget, running, _, _) = hydrate(&app, captured);
    assert_eq!(hydrated.value().speech().accepted_recordings().len(), 1);
    let prepared = hydrated.prepare_continuation().unwrap();
    let saved = prepared
        .value()
        .capture(
            CheckpointProfile::Authored,
            budget.reserve(Cohort::SavePayload, 4096).unwrap(),
        )
        .unwrap();
    let mut wire: Value = serde_json::from_slice(saved.value().bytes()).unwrap();
    drop(saved);
    drop(prepared);
    if wire["scheduler"]["version"] == 1 {
        wire["scheduler"] = json!({"version":2, "base":wire["scheduler"].take(),
            "continuation":{"load_retries":[], "resumed_context":null}});
    }
    if wire["night"]["version"] == 1 {
        wire["night"] = json!({"version":2, "base":wire["night"].take(), "load_retry_pending":false, "queued_incarnations":[]});
    }
    assert_eq!(wire["speech"]["version"], 2);
    drop(validate_wire(&app, &budget, &wire).expect("explicit complete V2 setup must be valid"));
    let before = super::tests_public::simulation_stamp(app.world());
    for path in [
        "/scheduler",
        "/scheduler/continuation",
        "/night",
        "/speech",
        "/speech/interrupted/0",
    ] {
        let fields: Vec<_> = wire
            .pointer(path)
            .unwrap()
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect();
        for field in fields {
            let mut missing = wire.clone();
            missing
                .pointer_mut(path)
                .unwrap()
                .as_object_mut()
                .unwrap()
                .remove(&field);
            assert!(
                validate_wire(&app, &budget, &missing).is_err(),
                "missing {path}/{field} accepted"
            );
            assert_eq!(budget.retained_bytes(), running.bytes());
        }
        let mut extra = wire.clone();
        extra
            .pointer_mut(path)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert("unknown_m2c_authority".into(), json!(true));
        assert!(
            validate_wire(&app, &budget, &extra).is_err(),
            "unknown field in {path} accepted"
        );
        assert_eq!(budget.retained_bytes(), running.bytes());
    }
    for (path, replacement) in [
        ("/speech/interrupted/0/receipts/0/ordinal", json!(0)),
        (
            "/speech/interrupted/0/receipts/0/outcome/code",
            json!("another_interruption"),
        ),
        (
            "/speech/interrupted/0/receipts/0/outcome/message",
            json!("Different outcome"),
        ),
        ("/speech/interrupted/0/receipts", json!([])),
        (
            "/speech/interrupted/0/state/accepted_recordings/0/receipt/ordinal",
            json!(u64::MAX),
        ),
    ] {
        let mut changed = wire.clone();
        *changed
            .pointer_mut(path)
            .expect("corruption must address an existing receipt field") = replacement;
        let result = validate_wire(&app, &budget, &changed);
        assert!(
            result.is_err(),
            "contradictory interruption accepted at {path}"
        );
        assert_eq!(budget.retained_bytes(), running.bytes());
    }
    let mut duplicated = wire.clone();
    let group = duplicated["speech"]["interrupted"][0].clone();
    duplicated["speech"]["interrupted"]
        .as_array_mut()
        .unwrap()
        .push(group);
    assert!(
        validate_wire(&app, &budget, &duplicated).is_err(),
        "same interrupted input retained twice"
    );
    assert_eq!(super::tests_public::simulation_stamp(app.world()), before);
    assert_eq!(budget.retained_bytes(), running.bytes());
}
