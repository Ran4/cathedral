use super::*;
use crate::checkpoint_storage::{
    self as storage,
    tests::{Fixture, TempDir, metadata, slot, wait_result},
};

/// Shared with independent review tests: immutable embedded definition recipe,
/// actual M3a durable save/load, and no Engine borrowed by the worker.
pub(crate) struct Harness {
    pub(crate) fixture: Fixture,
    storage: Option<storage::CheckpointStorage>,
    _directory: TempDir,
}
impl Harness {
    pub(crate) fn new() -> Self {
        let fixture = Fixture::new();
        let directory = TempDir::new();
        std::fs::File::open(directory.path().parent().unwrap())
            .unwrap()
            .sync_all()
            .unwrap();
        let storage =
            storage::CheckpointStorage::start(directory.path(), fixture.budget.clone()).unwrap();
        let save = storage.request_save(slot()).unwrap();
        storage
            .attach(save, fixture.capture(), metadata("preparation fixture"))
            .unwrap();
        assert!(matches!(
            wait_result(&storage, save),
            storage::Outcome::Saved(_)
        ));
        Self {
            fixture,
            storage: Some(storage),
            _directory: directory,
        }
    }
    pub(crate) fn load(&self) -> LoadedCheckpoint {
        let storage = self.storage.as_ref().unwrap();
        let load = storage
            .request_load(slot(), storage::LoadSelection::Active)
            .unwrap();
        let storage::Outcome::Loaded(value) = wait_result(storage, load) else {
            panic!("real storage load failed")
        };
        value
    }
    pub(crate) fn options(&self) -> PreparationOptions {
        PreparationOptions {
            recipe_upper_bytes: 4096,
            assets_upper_bytes: 32 * 1024 * 1024,
            host_definitions: self.fixture.definitions(),
            generation: RuntimeGeneration(2),
        }
    }
    pub(crate) fn submit(&self, service: &CheckpointPreparation) -> PreparationId {
        service
            .reserve_load(self.load(), self.options())
            .unwrap_or_else(|(error, _)| panic!("{error:?}"))
            .submit(|| Fixture::assets)
    }
}
impl Drop for Harness {
    fn drop(&mut self) {
        if let Some(storage) = self.storage.take() {
            drop(storage.shutdown().join());
        }
    }
}
pub(crate) fn wait(service: &CheckpointPreparation, id: PreparationId) -> PreparationOutcome {
    let until = Instant::now() + Duration::from_secs(20);
    loop {
        if let Some(value) = service.try_take(id) {
            return value;
        }
        assert!(Instant::now() < until, "preparation deadline");
        std::thread::sleep(Duration::from_millis(1));
    }
}
pub(crate) fn ready(service: &CheckpointPreparation, id: PreparationId) -> PreparedDelivery {
    match wait(service, id) {
        PreparationOutcome::Ready(value) => value,
        PreparationOutcome::Failed(error) => panic!("{error}"),
        _ => panic!("cancelled"),
    }
}

pub(crate) const CATEGORIES: [checkpoint::complete::CheckpointCategory; 16] = {
    use checkpoint::complete::CheckpointCategory::*;
    [
        Ledger,
        Operations,
        Backbone,
        Round,
        Climate,
        Knowledge,
        Law,
        Marks,
        Animals,
        Social,
        Continuity,
        Scheduler,
        Night,
        Speech,
        CognitionInputs,
        Host,
    ]
};
fn source_digests(fixture: &Harness) -> Vec<[u8; 32]> {
    use sha2::{Digest, Sha256};
    let captured = fixture.fixture.capture();
    CATEGORIES
        .iter()
        .map(|&category| Sha256::digest(captured.value().category(category)).into())
        .collect()
}
fn verify_all_owners(
    delivery: &PreparedDelivery,
    budget: &CheckpointBudget,
    expected: &[[u8; 32]],
) {
    for (category, expected) in CATEGORIES.into_iter().zip(expected) {
        let digest = delivery
            .hydrated()
            .unwrap()
            .category_digest(category, budget.reserve(Cohort::SavePayload, 4096).unwrap())
            .unwrap();
        assert_eq!(digest.value(), expected, "{category:?}");
    }
}

#[test]
fn real_file_worker_decodes_and_constructs_then_returns_prepared_owners() {
    let fixture = Harness::new();
    let service = CheckpointPreparation::start(fixture.fixture.budget.clone()).unwrap();
    let expected = source_digests(&fixture);
    let mut delivered = ready(&service, fixture.submit(&service));
    assert_eq!(
        delivered.decoded().unwrap().runtime_generation(),
        RuntimeGeneration(2)
    );
    println!("worker timings {:?}", delivered.timings());
    delivered.construct().unwrap();
    verify_all_owners(&delivered, &fixture.fixture.budget, &expected);
    delivered.prepare_continuation().unwrap();
    let prepared = delivered.prepared().unwrap();
    let capture = prepared
        .capture(
            checkpoint::complete::CheckpointProfile::Authored,
            fixture
                .fixture
                .budget
                .reserve(Cohort::SavePayload, 4096)
                .unwrap(),
        )
        .unwrap();
    assert_eq!(capture.value().world_identity(), prepared.world_identity());
    drop(capture);
    let id = delivered.id();
    drop(delivered);
    assert!(matches!(wait(&service, id), PreparationOutcome::Cancelled));
    assert!(service.join().is_ok());
}

#[test]
fn asset_factory_large_error_and_panic_are_bounded_before_terminal_retention() {
    let fixture = Harness::new();
    let service = CheckpointPreparation::start(fixture.fixture.budget.clone()).unwrap();
    for mode in 0..3 {
        let id = service
            .reserve_load(fixture.load(), fixture.options())
            .unwrap_or_else(|_| panic!("reserve"))
            .submit(|| {
                move || {
                    let mut reason = String::with_capacity(8 * 1024 * 1024);
                    reason.push_str(if mode == 0 {
                        "small length, large capacity"
                    } else {
                        "large"
                    });
                    if mode != 0 {
                        reason.extend(std::iter::repeat_n('x', 4 * 1024 * 1024));
                    }
                    if mode == 2 {
                        reason.truncate(5);
                        std::panic::panic_any(reason);
                    }
                    Err(checkpoint::CheckpointError {
                        owner: "fixture",
                        reason,
                    })
                }
            });
        let PreparationOutcome::Failed(error) = wait(&service, id) else {
            panic!("failure expected")
        };
        assert!(error.reason.capacity() <= 4096);
        assert!(error.reason.len() <= 4096);
    }
    assert!(service.join().is_ok());
}

#[test]
fn actual_saved_file_prepares_all_sixteen_owners_in_a_fresh_process() {
    let mut fixture = Harness::new();
    let expected = source_digests(&fixture);
    let definitions = fixture.options().host_definitions;
    let directory = fixture._directory.path();
    let expected_path = directory.join("owner-digests.json");
    std::fs::write(
        &expected_path,
        serde_json::to_vec(&(definitions, expected)).unwrap(),
    )
    .unwrap();
    drop(fixture.storage.take().unwrap().shutdown().join());
    let result = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "checkpoint_preparation::tests::preparation_fresh_process_child",
            "--ignored",
            "--nocapture",
        ])
        .env("ALIBI_M3B1_CHILD_DIRECTORY", directory)
        .output()
        .unwrap();
    println!(
        "fresh-process child stdout: {}",
        String::from_utf8_lossy(&result.stdout)
    );
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
}

#[test]
#[ignore = "invoked by real-file parent with a private directory"]
fn preparation_fresh_process_child() {
    let directory = std::path::PathBuf::from(
        std::env::var_os("ALIBI_M3B1_CHILD_DIRECTORY").expect("parent fixture"),
    );
    let (definitions, expected): (DefinitionsV1, Vec<[u8; 32]>) =
        serde_json::from_slice(&std::fs::read(directory.join("owner-digests.json")).unwrap())
            .unwrap();
    let budget = Arc::new(CheckpointBudget::default());
    let _running = budget
        .reserve(
            Cohort::Running,
            checkpoint::complete::RUNNING_AUTHORITY_ALLOWANCE_BYTES,
        )
        .unwrap();
    let storage = storage::CheckpointStorage::start(&directory, budget.clone()).unwrap();
    let load = storage
        .request_load(slot(), storage::LoadSelection::Active)
        .unwrap();
    let storage::Outcome::Loaded(loaded) = wait_result(&storage, load) else {
        panic!("load")
    };
    let service = CheckpointPreparation::start(budget.clone()).unwrap();
    let id = service
        .reserve_load(
            loaded,
            PreparationOptions {
                recipe_upper_bytes: 4096,
                assets_upper_bytes: 32 * 1024 * 1024,
                host_definitions: definitions,
                generation: RuntimeGeneration(2),
            },
        )
        .unwrap_or_else(|_| panic!("reserve"))
        .submit(|| Fixture::assets);
    let mut delivery = ready(&service, id);
    delivery.construct().unwrap();
    verify_all_owners(&delivery, &budget, &expected);
    delivery.prepare_continuation().unwrap();
    drop(delivery);
    assert!(matches!(wait(&service, id), PreparationOutcome::Cancelled));
    assert!(service.join().is_ok());
    drop(storage.shutdown().join());
    println!(
        "fresh-process real file: all sixteen actual owner digests equal; construction and M2c preparation without seed/poll"
    );
}

#[test]
fn delayed_factory_preserves_slot_and_cancelled_generation_cannot_construct() {
    let fixture = Harness::new();
    let service = CheckpointPreparation::start(fixture.fixture.budget.clone()).unwrap();
    let (entered, waiting) = crossbeam_channel::bounded(1);
    let (release, resume) = crossbeam_channel::bounded(1);
    let id = service
        .reserve_load(fixture.load(), fixture.options())
        .unwrap_or_else(|_| panic!("reserve"))
        .submit(|| {
            move || {
                entered.send(()).unwrap();
                resume.recv().unwrap();
                Fixture::assets()
            }
        });
    waiting.recv_timeout(Duration::from_secs(20)).unwrap();
    assert!(service.try_take(id).is_none());
    let saved = fixture.fixture.capture(); // old authority remains usable during real delayed decode
    service.cancel(id).unwrap();
    release.send(()).unwrap();
    assert!(matches!(wait(&service, id), PreparationOutcome::Cancelled));
    drop(saved);
    let mut delivery = ready(&service, fixture.submit(&service));
    service.cancel(delivery.id()).unwrap();
    assert!(!delivery.is_current());
    assert_eq!(delivery.construct(), Err(SubmitError::Closed));
    let id = delivery.id();
    drop(delivery);
    assert!(matches!(wait(&service, id), PreparationOutcome::Cancelled));
    assert!(service.join().is_ok());
}

#[test]
fn exact_installed_definitions_and_fresh_generation_are_required_by_real_load() {
    let fixture = Harness::new();
    let service = CheckpointPreparation::start(fixture.fixture.budget.clone()).unwrap();
    for mode in 0..2 {
        let mut options = fixture.options();
        if mode == 1 {
            options.generation = RuntimeGeneration(1);
        }
        let id = service
            .reserve_load(fixture.load(), options)
            .unwrap_or_else(|_| panic!("reserve"))
            .submit(|| {
                move || {
                    if mode == 0 {
                        // Change an actual installed host-image role in independently rebuilt assets.
                        let mut config = Fixture::configuration();
                        config.checkpoint_host_image = Some([42; 32]);
                        Fixture::assets_with_config(config)
                    } else {
                        Fixture::assets()
                    }
                }
            });
        assert!(matches!(wait(&service, id), PreparationOutcome::Failed(_)));
    }
    assert!(service.join().is_ok());
}

#[test]
fn reserved_builder_capture_is_admitted_and_cancelled_input_returns_after_shutdown() {
    let fixture = Harness::new();
    let budget = fixture.fixture.budget.clone();
    let service = CheckpointPreparation::start(budget.clone()).unwrap();
    let baseline = budget.retained_bytes();
    let permit = service
        .reserve_load(fixture.load(), fixture.options())
        .unwrap_or_else(|_| panic!("reserve"));
    let id = permit.id();
    assert!(budget.retained_bytes() > baseline + fixture.options().recipe_upper_bytes);
    service.close();
    drop(permit);
    assert!(matches!(wait(&service, id), PreparationOutcome::Cancelled));
    assert_eq!(budget.retained_bytes(), baseline);
    assert!(service.join().is_ok());

    let service = CheckpointPreparation::start(budget.clone()).unwrap();
    let mut options = fixture.options();
    options.recipe_upper_bytes = 1024 * 1024;
    let permit = service
        .reserve_load(fixture.load(), options)
        .unwrap_or_else(|_| panic!("reserve"));
    let admitted = budget.retained_bytes();
    let worker_budget = budget.clone();
    let id = permit.submit(|| {
        assert!(budget.retained_bytes() >= admitted);
        let captured = vec![7u8; 512 * 1024]; // allocated ONLY inside admitted builder
        move || {
            assert!(worker_budget.retained_bytes() >= admitted + options.assets_upper_bytes);
            assert_eq!(captured.len(), 512 * 1024);
            Fixture::assets()
        }
    });
    let delivery = ready(&service, id);
    drop(delivery);
    assert!(matches!(wait(&service, id), PreparationOutcome::Cancelled));
    assert!(service.join().is_ok());
}

#[test]
fn preparation_phase_samples_separate_host_work_from_worker_destruction() {
    let fixture = Harness::new();
    let service = CheckpointPreparation::start(fixture.fixture.budget.clone()).unwrap();
    let mut samples = Vec::new();
    for _ in 0..8 {
        let mut delivery = ready(&service, fixture.submit(&service));
        let worker = delivery.timings();
        let begin = Instant::now();
        delivery.construct().unwrap();
        let construction = begin.elapsed().as_secs_f64();
        let begin = Instant::now();
        delivery.prepare_continuation().unwrap();
        let continuation = begin.elapsed().as_secs_f64();
        let id = delivery.id();
        let begin = Instant::now();
        drop(delivery);
        let returned = begin.elapsed().as_secs_f64();
        let end = Instant::now() + Duration::from_secs(20);
        let disposal = loop {
            if let Some(value) = service.preparation_disposal_seconds(id) {
                break value;
            }
            assert!(Instant::now() < end);
            std::thread::sleep(Duration::from_millis(1));
        };
        assert!(matches!(wait(&service, id), PreparationOutcome::Cancelled));
        samples.push(serde_json::json!({"worker_phases":worker.worker_phases,"worker_total":worker.worker_total,
            "host_construction":construction,"host_continuation":continuation,"host_return":returned,"worker_disposal":disposal}));
    }
    println!(
        "M3b1 authored raw phase samples seconds={}; debug renderer-free fixture, no frame/p99 acceptance",
        serde_json::to_string(&samples).unwrap()
    );
    assert!(service.join().is_ok());
}
