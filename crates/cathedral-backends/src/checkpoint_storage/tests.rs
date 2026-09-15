use super::*;
use cathedral_sim::{
    checkpoint::{
        self, Admitted, CheckpointBudget, Cohort, HostTimeV1, Reservation,
        complete::{
            self, CheckpointProfile, CompleteCheckpointCandidate, InstalledCheckpointDefinitions,
            WorldIdentity,
        },
        host::{HostCheckpointSource, HudSlot, Nullable, RecordRef, RecordV1, ScalarsV1},
    },
    *,
};
use std::{
    path::{Path, PathBuf},
    sync::{
        Arc, OnceLock,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

pub(crate) struct TempDir(PathBuf);
impl TempDir {
    pub(crate) fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let p = std::env::temp_dir().join(format!(
            "alibi-m3a-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&p).unwrap();
        Self(p)
    }
    pub(crate) fn path(&self) -> &Path {
        &self.0
    }
}
impl Drop for TempDir {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}
struct Host(ScalarsV1);
impl HostCheckpointSource for Host {
    fn scalars(&self) -> checkpoint::Result<ScalarsV1> {
        Ok(self.0)
    }
    fn validate_boundary(&self) -> checkpoint::Result<()> {
        Ok(())
    }
    fn records(
        &self,
        f: &mut dyn FnMut(RecordRef<'_>) -> checkpoint::Result<()>,
    ) -> checkpoint::Result<()> {
        self.complete_records(f)
    }
    fn complete_records<'a>(
        &'a self,
        f: &mut dyn FnMut(RecordRef<'a>) -> checkpoint::Result<()>,
    ) -> checkpoint::Result<()> {
        f(RecordV1::Draft { text: "" })?;
        f(RecordV1::SelectedItem {
            item: Nullable(None),
        })?;
        for slot in [
            HudSlot::Subtitle,
            HudSlot::Inventory,
            HudSlot::OfferCard,
            HudSlot::LawStanding,
            HudSlot::JournalStanding,
            HudSlot::FocusHint,
        ] {
            f(RecordV1::Hud {
                slot,
                text: "",
                remaining: Nullable(None),
            })?;
        }
        f(RecordV1::ChalkHold {
            intent: Nullable(None),
        })?;
        f(RecordV1::ChalkPen { present: false })
    }
}
struct Unavailable;
impl Cognition for Unavailable {
    fn request(&mut self, _: String) -> Result<RequestId, CognitionBusy> {
        Err(CognitionBusy)
    }
}
pub(crate) struct Fixture {
    engine: Engine,
    host: Host,
    pub(crate) budget: Arc<CheckpointBudget>,
    _running: Reservation,
}
impl Fixture {
    pub(crate) fn configuration() -> EngineConfig {
        static IMAGE: OnceLock<[u8; 32]> = OnceLock::new();
        let image = *IMAGE.get_or_init(|| {
            use sha2::{Digest, Sha256};
            use std::io::Read;
            let mut f = std::fs::File::open("/proc/self/exe").unwrap();
            let mut hash = Sha256::new();
            let mut bytes = [0u8; 65536];
            loop {
                let n = f.read(&mut bytes).unwrap();
                if n == 0 {
                    break;
                }
                hash.update(&bytes[..n]);
            }
            hash.finalize().into()
        });
        EngineConfig {
            checkpoint_host_image: Some(image),
            checkpoint_world_identity: Some(WorldIdentity::from_bytes([9; 16]).unwrap()),
            knowledge_enabled: false,
            marks_enabled: false,
            ..Default::default()
        }
    }
    pub(crate) fn new() -> Self {
        let config = Self::configuration();
        let clock = config.clock;
        let seed = WorldSeed::from_json_str(include_str!(
            "../../../cathedral-sim/tests/fixtures/demo_seed.json"
        ))
        .unwrap();
        let mut engine = Engine::new(
            config,
            &seed,
            areas::AreaMap::from_json_str(include_str!("../../../../assets/world/areas.json"))
                .unwrap(),
            SoundCatalog::from_toml_str(include_str!("../../../../assets/sounds/catalog.toml"))
                .unwrap(),
            PromptEnv::new(
                include_str!("../../../../assets/prompts/turn.j2"),
                include_str!("../../../../assets/prompts/night.j2"),
                include_str!("../../../../assets/prompts/strings.toml"),
            )
            .unwrap(),
            Box::new(Unavailable),
            Box::new(NullTranscription),
            Box::new(NullTts),
            Box::new(NullSight),
            Capabilities::new(true, false, false, false, false, TtsBackendKind::Off),
            (Vec3::new(0.0, f64::from(0.91_f32), 111.0), 0.0),
            0,
            0.0,
        )
        .unwrap();
        // Complete the ordinary initial Ready/publication boundary. Storage
        // never polls; this belongs to the fixture's normal host startup.
        engine.poll(0.0, Vec::new());
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../cathedral-sim/tests/fixtures/checkpoint_host/initial-v1.json"
        ))
        .unwrap();
        let mut s: ScalarsV1 = serde_json::from_value(fixture["scalars"].clone()).unwrap();
        let p = engine.world().characters[&engine.config().player_id].position_m();
        s.controller.current = [p.x as f32, p.y as f32, p.z as f32];
        s.controller.previous = s.controller.current;
        s.controller.yaw = 0.0;
        s.controller.velocity = [0.0; 3];
        s.spatial.sequence = engine.world().spatial_sequence as u64;
        s.spatial.last_position = Nullable(Some(s.controller.current));
        s.spatial.last_yaw = Nullable(Some(0.0));
        s.boundary.generation = 1;
        s.boundary.physical_sequence = s.spatial.sequence;
        s.boundary.input_watermark = 0;
        s.boundary.issued = 0;
        s.time.accepted = HostTimeV1::from_accepted(
            timeline::AcceptedTime::default(),
            Duration::from_nanos(8_333_333),
            Duration::ZERO,
            0.0,
        )
        .unwrap();
        s.time.virtual_elapsed = Duration::ZERO;
        s.time.virtual_delta = Duration::ZERO;
        s.time.fixed_elapsed = Duration::ZERO;
        s.time.fixed_delta = Duration::ZERO;
        s.time.last_frame = Nullable(None);
        let time = clock.at(0.0);
        s.clock.day = time.day;
        s.clock.fraction = time.fraction;
        s.clock.office = time.office.into();
        s.clock.weekday = time.weekday.into();
        s.clock.brightness = clock.brightness(0.0);
        s.clock.scale = clock.scale();
        s.clock.seconds_per_day = clock.seconds_per_day();
        let budget = Arc::new(CheckpointBudget::default());
        let running = budget
            .reserve(Cohort::Running, complete::RUNNING_AUTHORITY_ALLOWANCE_BYTES)
            .unwrap();
        Self {
            engine,
            host: Host(s),
            budget,
            _running: running,
        }
    }
    pub(crate) fn definitions(&self) -> checkpoint::host::DefinitionsV1 {
        self.host.0.definitions
    }
    pub(crate) fn assets() -> checkpoint::Result<complete::HydrationAssets> {
        Self::assets_with_config(Self::configuration())
    }
    pub(crate) fn assets_with_config(
        config: EngineConfig,
    ) -> checkpoint::Result<complete::HydrationAssets> {
        let seed = WorldSeed::from_json_str(include_str!(
            "../../../cathedral-sim/tests/fixtures/demo_seed.json"
        ))
        .unwrap();
        let areas =
            AreaMap::from_json_str(include_str!("../../../../assets/world/areas.json")).unwrap();
        let adjacency = Arc::new(knowledge::AreaAdjacency::build(&areas));
        let assets = complete::HydrationWorldAssets {
            areas,
            sounds: SoundCatalog::from_toml_str(include_str!(
                "../../../../assets/sounds/catalog.toml"
            ))
            .unwrap(),
            items: ItemCatalog::embedded(),
            nav: None,
            shelters: config.shelters.clone(),
            marks: Arc::new(marks::MarkCatalog::default()),
            facts: Arc::new(knowledge::FactCatalog::default()),
            salience: Arc::new(knowledge::SalienceTable::default()),
            area_adjacency: adjacency,
        };
        complete::HydrationAssets::new(
            &seed,
            config,
            PromptEnv::new(
                include_str!("../../../../assets/prompts/turn.j2"),
                include_str!("../../../../assets/prompts/night.j2"),
                include_str!("../../../../assets/prompts/strings.toml"),
            )
            .unwrap(),
            assets,
        )
    }
    pub(crate) fn capture(&self) -> Admitted<CompleteCheckpointCandidate> {
        complete::capture(
            &self.engine,
            &self.host,
            CheckpointProfile::Authored,
            self.budget.reserve(Cohort::SavePayload, 4096).unwrap(),
        )
        .unwrap()
    }
    pub(crate) fn validate(
        &self,
        loaded: LoadedCheckpoint,
    ) -> Admitted<CompleteCheckpointCandidate> {
        self.try_validate(loaded).unwrap()
    }
    fn try_validate(
        &self,
        mut loaded: LoadedCheckpoint,
    ) -> checkpoint::Result<Admitted<CompleteCheckpointCandidate>> {
        loaded.input.prepare_definition_resolution().unwrap();
        let definitions = InstalledCheckpointDefinitions::from_engine(
            &self.engine,
            self.host.0.definitions,
            timeline::LogicalTime::new(0.0).unwrap(),
        )
        .unwrap();
        complete::validate_owned_definitions_observed(loaded.input, definitions, &mut |_| {})
    }
}

const PUBLICATION_PHASES: &[Phase] = &[
    Phase::JournalWrite,
    Phase::JournalFlush,
    Phase::JournalReplace,
    Phase::JournalSync,
    Phase::PayloadWrite,
    Phase::PayloadValidate,
    Phase::PayloadFlush,
    Phase::PayloadPublish,
    Phase::PayloadSync,
    Phase::RecoveryWrite,
    Phase::RecoveryFlush,
    Phase::RecoveryReplace,
    Phase::RecoverySync,
    Phase::ActiveWrite,
    Phase::ActiveFlush,
    Phase::ActiveReplace,
    Phase::ActiveSync,
    Phase::Cleanup,
    Phase::CleanupSync,
    Phase::PendingRemove,
    Phase::PendingSync,
];
fn baseline(
    store: &mut disk::Store,
    payload: &Admitted<CompleteCheckpointCandidate>,
) -> (SlotReference, SlotReference) {
    let b = store.save(&slot(), id(1), metadata("B"), payload).unwrap();
    let a = store.save(&slot(), id(2), metadata("A"), payload).unwrap();
    (b, a)
}
fn fail_once(phase: Phase, edge: disk::Edge) -> disk::Hook {
    let failed = std::sync::atomic::AtomicBool::new(false);
    Arc::new(move |p, e| {
        if p == phase && e == edge && !failed.swap(true, Ordering::SeqCst) {
            Err(std::io::Error::from_raw_os_error(libc::ENOSPC))
        } else {
            Ok(())
        }
    })
}
fn assert_title(fixture: &Fixture, loaded: LoadedCheckpoint, title: &str) {
    assert_eq!(loaded.reference.metadata().title(), title);
    drop(fixture.validate(loaded));
}
fn known_payloads(path: &Path) -> usize {
    std::fs::read_dir(path)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|name| name.contains(".gen-") && name.ends_with(".json"))
        .count()
}

#[test]
fn storage_every_returned_publication_fault_preserves_actual_third_save_predecessor() {
    let fixture = Fixture::new();
    let payload = fixture.capture();
    for &phase in PUBLICATION_PHASES {
        for edge in [disk::Edge::Before, disk::Edge::After] {
            let temp = TempDir::new();
            let mut store = disk::Store::open(temp.path()).unwrap();
            let (_, a) = baseline(&mut store, &payload);
            store.hook = Some(fail_once(phase, edge));
            let error = store
                .save(&slot(), id(3), metadata("C"), &payload)
                .unwrap_err();
            assert_eq!(error.phase, phase, "{phase:?}/{edge:?}");
            assert!(
                temp.path()
                    .join(disk::payload_file(&slot(), a.generation()))
                    .exists()
            );
            assert!(
                store
                    .load(&slot(), LoadSelection::Active, &fixture.budget)
                    .is_err()
            );
            assert_title(
                &fixture,
                store
                    .load(&slot(), LoadSelection::Previous, &fixture.budget)
                    .unwrap(),
                "A",
            );
            assert!(store.save(&slot(), id(4), metadata("D"), &payload).is_err());
            assert!(known_payloads(temp.path()) <= 3);
            store.hook = None;
            assert_eq!(
                store.recover(&slot(), id(5)).unwrap().unwrap().generation(),
                a.generation()
            );
            assert_title(
                &fixture,
                store
                    .load(&slot(), LoadSelection::Active, &fixture.budget)
                    .unwrap(),
                "A",
            );
            store.save(&slot(), id(6), metadata("D"), &payload).unwrap();
            assert_title(
                &fixture,
                store
                    .load(&slot(), LoadSelection::Previous, &fixture.budget)
                    .unwrap(),
                "A",
            );
            assert_eq!(known_payloads(temp.path()), 2);
            println!(
                "returned-fault {phase:?} {edge:?}: A retained, recovery durable, next save bounded"
            );
        }
    }
}

#[test]
fn storage_interrupted_recovery_and_repeated_cleanup_failure_keep_original_acknowledgement() {
    let fixture = Fixture::new();
    let payload = fixture.capture();
    let phases = [
        Phase::JournalWrite,
        Phase::JournalFlush,
        Phase::JournalReplace,
        Phase::JournalSync,
        Phase::RecoveryWrite,
        Phase::RecoveryFlush,
        Phase::RecoveryReplace,
        Phase::RecoverySync,
        Phase::ActiveWrite,
        Phase::ActiveFlush,
        Phase::ActiveReplace,
        Phase::ActiveSync,
        Phase::Cleanup,
        Phase::CleanupSync,
        Phase::PendingRemove,
        Phase::PendingSync,
    ];
    for phase in phases {
        let temp = TempDir::new();
        let mut store = disk::Store::open(temp.path()).unwrap();
        baseline(&mut store, &payload);
        store.hook = Some(fail_once(Phase::ActiveSync, disk::Edge::After));
        store
            .save(&slot(), id(3), metadata("C"), &payload)
            .unwrap_err();
        for attempt in 0..2 {
            store.hook = Some(fail_once(phase, disk::Edge::Before));
            assert_eq!(
                store.recover(&slot(), id(4 + attempt)).unwrap_err().phase,
                phase
            );
            assert_title(
                &fixture,
                store
                    .load(&slot(), LoadSelection::Previous, &fixture.budget)
                    .unwrap(),
                "A",
            );
            drop(store);
            store = disk::Store::open(temp.path()).unwrap();
            // Missing pending at final-sync failure is conservative: explicit
            // Previous still selects A and explicit recovery remains possible.
            assert_title(
                &fixture,
                store
                    .load(&slot(), LoadSelection::Previous, &fixture.budget)
                    .unwrap(),
                "A",
            );
            assert!(known_payloads(temp.path()) <= 3);
        }
        store.hook = None;
        store.recover(&slot(), id(8)).unwrap();
        assert_title(
            &fixture,
            store
                .load(&slot(), LoadSelection::Active, &fixture.budget)
                .unwrap(),
            "A",
        );
        assert!(known_payloads(temp.path()) <= 2);
    }
}

#[test]
#[ignore = "fresh-process helper invoked only by the bounded storage death matrix"]
fn storage_process_child() {
    let path = PathBuf::from(std::env::var_os("ALIBI_M3A_CHILD_DIR").expect("child dir"));
    let mode = std::env::var("ALIBI_M3A_CHILD_MODE").expect("child mode");
    let fixture = Fixture::new();
    let mut store = disk::Store::open(&path).unwrap();
    if mode == "read" {
        let loaded = store
            .load(&slot(), LoadSelection::Active, &fixture.budget)
            .unwrap();
        assert_title(&fixture, loaded, "A");
        println!("fresh same-image M2 validation passed");
        return;
    }
    let phase: Phase =
        serde_json::from_str(&std::env::var("ALIBI_M3A_CHILD_PHASE").unwrap()).unwrap();
    store.hook = Some(Arc::new(move |p, e| {
        if p == phase && e == disk::Edge::After {
            std::process::exit(86);
        }
        Ok(())
    }));
    if mode == "recover" {
        store.recover(&slot(), id(4)).unwrap();
    } else {
        store
            .save(&slot(), id(3), metadata("C"), &fixture.capture())
            .unwrap();
    }
    panic!("death point not reached");
}
fn child(path: &Path, mode: &str, phase: Option<Phase>) -> std::process::Output {
    let mut command = std::process::Command::new(std::env::current_exe().unwrap());
    command
        .args([
            "--exact",
            "checkpoint_storage::tests::storage_process_child",
            "--ignored",
            "--nocapture",
        ])
        .env("ALIBI_M3A_CHILD_DIR", path)
        .env("ALIBI_M3A_CHILD_MODE", mode);
    if let Some(phase) = phase {
        command.env(
            "ALIBI_M3A_CHILD_PHASE",
            serde_json::to_string(&phase).unwrap(),
        );
    }
    let output = command.output().unwrap();
    println!(
        "{}",
        serde_json::json!({"subprocess": "m3a-storage", "mode": mode, "phase": phase,
        "exit_code": output.status.code(), "stdout": std::str::from_utf8(&output.stdout).expect("lossless child stdout"), "stderr": std::str::from_utf8(&output.stderr).expect("lossless child stderr")})
    );
    output
}

#[test]
fn storage_fresh_process_death_at_every_publication_step_and_recovery_step() {
    let fixture = Fixture::new();
    let payload = fixture.capture();
    for mode in ["publish", "recover"] {
        for &phase in PUBLICATION_PHASES {
            if mode == "recover"
                && matches!(
                    phase,
                    Phase::PayloadWrite
                        | Phase::PayloadValidate
                        | Phase::PayloadFlush
                        | Phase::PayloadPublish
                        | Phase::PayloadSync
                )
            {
                continue;
            }
            let temp = TempDir::new();
            let mut store = disk::Store::open(temp.path()).unwrap();
            baseline(&mut store, &payload);
            if mode == "recover" {
                store.hook = Some(fail_once(Phase::ActiveSync, disk::Edge::After));
                store
                    .save(&slot(), id(3), metadata("C"), &payload)
                    .unwrap_err();
            }
            drop(store);
            assert_eq!(
                child(temp.path(), mode, Some(phase)).status.code(),
                Some(86),
                "{mode}/{phase:?}"
            );
            let mut store = disk::Store::open(temp.path()).unwrap();
            let pending = temp
                .path()
                .join(disk::slot_file(&slot(), "pending"))
                .exists();
            if pending {
                assert_title(
                    &fixture,
                    store
                        .load(&slot(), LoadSelection::Previous, &fixture.budget)
                        .unwrap(),
                    "A",
                );
                assert!(
                    store
                        .save(&slot(), id(9), metadata("unadmitted"), &payload)
                        .is_err()
                );
                store.recover(&slot(), id(10)).unwrap();
            } else {
                // Before the journal replacement active is still A. After the
                // final pending removal active may be C: select its retained A.
                let loaded = store
                    .load(&slot(), LoadSelection::Active, &fixture.budget)
                    .unwrap();
                let was_c = loaded.reference.metadata().title() == "C";
                drop(fixture.validate(loaded));
                if was_c {
                    store.recover(&slot(), id(10)).unwrap();
                }
            }
            assert_title(
                &fixture,
                store
                    .load(&slot(), LoadSelection::Active, &fixture.budget)
                    .unwrap(),
                "A",
            );
            assert!(known_payloads(temp.path()) <= 2);
            drop(store);
            assert!(child(temp.path(), "read", None).status.success());
            println!(
                "process-death {mode} {phase:?}: same-image predecessor validates in a fresh reader"
            );
        }
    }
}

#[test]
fn storage_real_partial_enospc_write_and_interrupted_first_save_recover_without_orphan_guessing() {
    let fixture = Fixture::new();
    let payload = fixture.capture();
    for first in [false, true] {
        let temp = TempDir::new();
        let mut store = disk::Store::open(temp.path()).unwrap();
        if !first {
            baseline(&mut store, &payload);
        }
        std::fs::write(
            temp.path().join("unknown-orphan.json"),
            payload.value().bytes(),
        )
        .unwrap();
        store.partial_payload_bytes = Some(37);
        let err = store
            .save(&slot(), id(3), metadata("partial"), &payload)
            .unwrap_err();
        assert_eq!(err.phase, Phase::PayloadWrite);
        assert_eq!(err.kind, std::io::ErrorKind::StorageFull);
        let temp_payload = temp
            .path()
            .join(format!("{}.tmp", disk::payload_file(&slot(), id(3))));
        assert_eq!(
            std::fs::read(&temp_payload).unwrap(),
            &payload.value().bytes()[..37]
        );
        drop(store);
        let mut store = disk::Store::open(temp.path()).unwrap();
        if first {
            assert!(
                store
                    .load(&slot(), LoadSelection::Previous, &fixture.budget)
                    .is_err()
            );
        } else {
            assert_title(
                &fixture,
                store
                    .load(&slot(), LoadSelection::Previous, &fixture.budget)
                    .unwrap(),
                "A",
            );
        }
        let restored = store.recover(&slot(), id(4)).unwrap();
        assert_eq!(restored.is_none(), first);
        assert!(!temp_payload.exists());
        assert!(temp.path().join("unknown-orphan.json").exists());
        store
            .save(&slot(), id(5), metadata("after recovery"), &payload)
            .unwrap();
        assert_title(
            &fixture,
            store
                .load(&slot(), LoadSelection::Active, &fixture.budget)
                .unwrap(),
            "after recovery",
        );
    }
}

fn rewrite_payload(path: &Path, reference: &SlotReference, bytes: &[u8]) {
    let mut reference = reference.clone();
    reference.payload_bytes = bytes.len() as u64;
    reference.payload_sha256 = disk::hash(bytes);
    std::fs::write(
        path.join(disk::payload_file(&slot(), reference.generation)),
        bytes,
    )
    .unwrap();
    let checksum = disk::hash(&serde_json::to_vec(&reference).unwrap());
    std::fs::write(
        path.join(disk::slot_file(&slot(), "active")),
        serde_json::to_vec(&serde_json::json!({"body":reference,"sha256":checksum})).unwrap(),
    )
    .unwrap();
}
#[test]
fn storage_checksum_length_reference_and_envelope_refusals_leave_previous_usable() {
    let fixture = Fixture::new();
    let payload = fixture.capture();
    for mutation in [
        "checksum",
        "truncated",
        "trailing",
        "version",
        "unknown",
        "manifest",
        "metadata",
        "reference",
    ] {
        let temp = TempDir::new();
        let mut store = disk::Store::open(temp.path()).unwrap();
        let (_, a) = baseline(&mut store, &payload);
        let mut bytes = payload.value().bytes().to_vec();
        match mutation {
            "checksum" => {
                bytes[0] = b' ';
                std::fs::write(
                    temp.path().join(disk::payload_file(&slot(), a.generation)),
                    &bytes,
                )
                .unwrap();
            }
            "truncated" => {
                bytes.pop();
                std::fs::write(
                    temp.path().join(disk::payload_file(&slot(), a.generation)),
                    &bytes,
                )
                .unwrap();
            }
            "trailing" => {
                bytes.push(b' ');
                std::fs::write(
                    temp.path().join(disk::payload_file(&slot(), a.generation)),
                    &bytes,
                )
                .unwrap();
            }
            "reference" => {
                std::fs::write(
                    temp.path().join(disk::slot_file(&slot(), "active")),
                    b"{broken",
                )
                .unwrap();
            }
            other => {
                let mut value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
                match other {
                    "version" => value["version"] = 2.into(),
                    "unknown" => value["unowned_category"] = true.into(),
                    "manifest" => {
                        value["manifest"]["host_image"] = serde_json::to_value([0u8; 32]).unwrap()
                    }
                    "metadata" => {
                        value["world_identity"] = serde_json::to_value([8u8; 16]).unwrap()
                    }
                    _ => unreachable!(),
                }
                rewrite_payload(temp.path(), &a, &serde_json::to_vec(&value).unwrap());
            }
        }
        let result = store.load(&slot(), LoadSelection::Active, &fixture.budget);
        if mutation == "manifest" {
            // Storage's framing check intentionally grants no installed-content
            // compatibility. The real pure M2 boundary performs this refusal.
            assert!(fixture.try_validate(result.unwrap()).is_err());
        } else {
            assert!(result.is_err(), "{mutation}");
        }
        assert_title(
            &fixture,
            store
                .load(&slot(), LoadSelection::Previous, &fixture.budget)
                .unwrap(),
            "B",
        );
    }
}

#[test]
fn storage_slow_worker_queue_terminals_reverse_capture_and_cancellation_are_bounded() {
    let fixture = Fixture::new();
    let temp = TempDir::new();
    let (entered, waiting) = crossbeam_channel::bounded(1);
    let (release, resume) = crossbeam_channel::bounded(1);
    let gate = std::sync::atomic::AtomicBool::new(false);
    let hook = Arc::new(move |phase, edge| {
        if phase == Phase::PayloadWrite
            && edge == disk::Edge::Before
            && !gate.swap(true, Ordering::SeqCst)
        {
            entered.send(()).unwrap();
            resume.recv().unwrap();
        }
        Ok(())
    });
    let service =
        CheckpointStorage::start_hooked(temp.path(), fixture.budget.clone(), hook).unwrap();
    let first = service.request_save(slot()).unwrap();
    let second = service.request_save(slot()).unwrap();
    let (error, payload, returned_metadata) = service
        .attach(second, fixture.capture(), metadata("newer"))
        .unwrap_err();
    assert_eq!(returned_metadata, metadata("newer"));
    assert_eq!(error, SubmitError::WrongTurn);
    service.attach(first, payload, metadata("older")).unwrap();
    waiting.recv_timeout(Duration::from_secs(10)).unwrap();
    assert!(fixture.budget.reserve(Cohort::SavePayload, 1).is_err());
    assert_eq!(service.next_capture(), None);
    let mut cancelled = Vec::new();
    for _ in 2..OPERATION_CAPACITY {
        cancelled.push(service.request_save(slot()).unwrap());
    }
    assert_eq!(service.retained_operations(), OPERATION_CAPACITY);
    assert_eq!(
        service.request_load(slot(), LoadSelection::Active),
        Err(SubmitError::Full)
    );
    assert_eq!(service.cancel(first), Err(SubmitError::TooLate));
    for &id in &cancelled {
        service.cancel(id).unwrap();
    }
    release.send(()).unwrap();
    assert_eq!(
        saved(wait_result(&service, first)).metadata().title(),
        "older"
    );
    assert_eq!(service.next_capture(), Some(second));
    let actual_capture = SaveMetadata::new("newer", 1_789_430_500, "The next square").unwrap();
    service
        .attach(second, fixture.capture(), actual_capture.clone())
        .unwrap();
    assert_eq!(
        saved(wait_result(&service, second)).metadata(),
        &actual_capture
    );
    for id in cancelled {
        assert!(matches!(wait_result(&service, id), Outcome::Cancelled));
    }
    let load = service.request_load(slot(), LoadSelection::Active).unwrap();
    let Outcome::Loaded(loaded) = wait_result(&service, load) else {
        panic!("load failed");
    };
    assert_title(&fixture, loaded, "newer");
    let mut unread = Vec::new();
    for _ in 0..OPERATION_CAPACITY {
        let id = service.request_save(slot()).unwrap();
        service.cancel(id).unwrap();
        unread.push(id);
    }
    // Even completed, unread Cancelled results reserve all promised slots.
    assert_eq!(service.request_recovery(slot()), Err(SubmitError::Full));
    let shutdown = service.shutdown();
    for id in unread {
        let until = Instant::now() + Duration::from_secs(10);
        loop {
            if let Some(outcome) = shutdown.take_result(id) {
                assert!(matches!(outcome, Outcome::Cancelled));
                break;
            }
            assert!(Instant::now() < until);
            std::thread::sleep(Duration::from_millis(1));
        }
    }
    drop(shutdown.join());
    assert_eq!(
        fixture.budget.retained_bytes(),
        complete::RUNNING_AUTHORITY_ALLOWANCE_BYTES
    );
}

#[test]
fn storage_service_admission_identity_exhaustion_and_directory_lock_are_explicit() {
    let temp = TempDir::new();
    let budget = Arc::new(CheckpointBudget::default());
    assert_eq!(
        CheckpointStorage::start(temp.path(), budget.clone())
            .err()
            .unwrap()
            .phase,
        Phase::Admission
    );
    let running = budget
        .reserve(
            Cohort::Running,
            checkpoint::MAX_RESIDENT_BYTES - SERVICE_ALLOWANCE_BYTES + 1,
        )
        .unwrap();
    let before = budget.retained_bytes();
    assert_eq!(
        CheckpointStorage::start(temp.path(), budget.clone())
            .err()
            .unwrap()
            .phase,
        Phase::Admission
    );
    assert_eq!(budget.retained_bytes(), before);
    assert_eq!(budget.peak_retained_bytes(), before);
    drop(running);

    let fixture = Fixture::new();
    let other = Fixture::new();
    let service = CheckpointStorage::start(temp.path(), fixture.budget.clone()).unwrap();
    let op = service.request_save(slot()).unwrap();
    let (error, owner, returned_metadata) = service
        .attach(op, other.capture(), metadata("wrong budget"))
        .unwrap_err();
    assert_eq!(returned_metadata, metadata("wrong budget"));
    assert_eq!(error, SubmitError::WrongBudget);
    assert!(other.budget.owns_admitted(&owner, Cohort::SavePayload));
    drop(owner);
    service
        .attach(op, fixture.capture(), metadata("valid budget"))
        .unwrap();
    saved(wait_result(&service, op));
    let contending = CheckpointStorage::start(temp.path(), fixture.budget.clone()).unwrap();
    let locked = contending
        .request_load(slot(), LoadSelection::Active)
        .unwrap();
    let Outcome::Failed(error) = wait_result(&contending, locked) else {
        panic!("concurrent directory owner admitted");
    };
    assert_eq!(error.phase, Phase::Open);
    drop(contending.shutdown().join());
    service.set_next_for_test(u64::MAX - 1);
    let last = service.request_save(slot()).unwrap();
    assert_eq!(service.request_save(slot()), Err(SubmitError::Exhausted));
    assert_eq!(service.retained_operations(), 1);
    service.cancel(last).unwrap();
    assert!(matches!(wait_result(&service, last), Outcome::Cancelled));
    drop(service.shutdown().join());
    assert_eq!(
        fixture.budget.retained_bytes(),
        complete::RUNNING_AUTHORITY_ALLOWANCE_BYTES
    );
}

#[test]
fn storage_inspection_capacity_pressure_is_retryable_and_never_labels_valid_bytes_corrupt() {
    let mut fixture = Fixture::new();
    let temp = TempDir::new();
    let service = CheckpointStorage::start(temp.path(), fixture.budget.clone()).unwrap();
    let save = service.request_save(slot()).unwrap();
    service
        .attach(save, fixture.capture(), metadata("valid under pressure"))
        .unwrap();
    let reference = saved(wait_result(&service, save));
    let payload_path = temp
        .path()
        .join(disk::payload_file(&slot(), reference.generation()));
    let before = std::fs::read(&payload_path).unwrap();
    fixture
        ._running
        .resize(
            checkpoint::MAX_RESIDENT_BYTES
                - SERVICE_ALLOWANCE_BYTES
                - reference.payload_bytes() as usize
                - 4096
                - 1024,
        )
        .unwrap();
    let charged = fixture.budget.retained_bytes();
    let request = service.request_load(slot(), LoadSelection::Active).unwrap();
    let Outcome::Failed(error) = wait_result(&service, request) else {
        panic!("inspection scratch exceeded shared capacity");
    };
    assert_eq!(error.phase, Phase::Admission);
    assert_eq!(error.kind, std::io::ErrorKind::WouldBlock);
    assert_eq!(fixture.budget.retained_bytes(), charged);
    assert_eq!(std::fs::read(&payload_path).unwrap(), before);
    fixture
        ._running
        .resize(complete::RUNNING_AUTHORITY_ALLOWANCE_BYTES)
        .unwrap();
    let request = service.request_load(slot(), LoadSelection::Active).unwrap();
    let Outcome::Loaded(loaded) = wait_result(&service, request) else {
        panic!("valid unchanged file could not retry after capacity release");
    };
    assert_eq!(loaded.reference, reference);
    drop(fixture.validate(loaded));
    drop(service.shutdown().join());
    assert_eq!(
        fixture.budget.retained_bytes(),
        complete::RUNNING_AUTHORITY_ALLOWANCE_BYTES
    );
}

#[test]
fn storage_symlink_payload_and_unknown_filesystem_do_not_gain_file_authority() {
    let fixture = Fixture::new();
    let temp = TempDir::new();
    let mut store = disk::Store::open(temp.path()).unwrap();
    let reference = store
        .save(&slot(), id(1), metadata("actual"), &fixture.capture())
        .unwrap();
    let payload_path = temp
        .path()
        .join(disk::payload_file(&slot(), reference.generation));
    let sentinel = temp.path().join("sentinel");
    std::fs::write(&sentinel, b"do not follow me").unwrap();
    std::fs::remove_file(&payload_path).unwrap();
    std::os::unix::fs::symlink(&sentinel, &payload_path).unwrap();
    assert_eq!(
        store
            .load(&slot(), LoadSelection::Active, &fixture.budget)
            .unwrap_err()
            .phase,
        Phase::Read
    );
    assert_eq!(std::fs::read(&sentinel).unwrap(), b"do not follow me");
    if Path::new("/dev/shm").is_dir() {
        let error = disk::Store::open(Path::new("/dev/shm")).err().unwrap();
        assert_eq!(error.phase, Phase::Open);
        assert_eq!(error.kind, std::io::ErrorKind::Unsupported);
    }
}

#[test]
fn storage_worker_unwind_keeps_a_terminal_and_recoverable_original_journal() {
    let fixture = Fixture::new();
    let temp = TempDir::new();
    let fired = std::sync::atomic::AtomicBool::new(false);
    let hook = Arc::new(move |phase, edge| {
        if phase == Phase::PayloadFlush
            && edge == disk::Edge::After
            && !fired.swap(true, Ordering::SeqCst)
        {
            panic!("intentional storage-worker unwind witness");
        }
        Ok(())
    });
    let service =
        CheckpointStorage::start_hooked(temp.path(), fixture.budget.clone(), hook).unwrap();
    let first = service.request_save(slot()).unwrap();
    service
        .attach(first, fixture.capture(), metadata("interrupted first save"))
        .unwrap();
    let Outcome::Failed(error) = wait_result(&service, first) else {
        panic!("worker unwind lost its failure terminal");
    };
    assert_eq!(error.phase, Phase::Worker);
    assert!(fixture.budget.reserve(Cohort::SavePayload, 4096).is_ok());
    let recovery = service.request_recovery(slot()).unwrap();
    assert!(matches!(
        wait_result(&service, recovery),
        Outcome::Recovered(None)
    ));
    let next = service.request_save(slot()).unwrap();
    service
        .attach(next, fixture.capture(), metadata("after unwind"))
        .unwrap();
    assert_eq!(
        saved(wait_result(&service, next)).metadata().title(),
        "after unwind"
    );
    drop(service.shutdown().join());
}

#[test]
fn storage_dropping_a_handle_never_joins_blocked_disk_or_releases_its_live_charge() {
    let fixture = Fixture::new();
    let temp = TempDir::new();
    let (entered, blocked) = crossbeam_channel::bounded(1);
    let (release, resume) = crossbeam_channel::bounded(1);
    let hook = Arc::new(move |phase, edge| {
        if phase == Phase::PayloadWrite && edge == disk::Edge::Before {
            entered.send(()).unwrap();
            resume.recv().unwrap();
        }
        Ok(())
    });
    let service =
        CheckpointStorage::start_hooked(temp.path(), fixture.budget.clone(), hook).unwrap();
    let operation = service.request_save(slot()).unwrap();
    let payload = fixture.capture();
    let retained = complete::RUNNING_AUTHORITY_ALLOWANCE_BYTES
        + SERVICE_ALLOWANCE_BYTES
        + payload.reserved_bytes();
    service
        .attach(operation, payload, metadata("finishes after abandonment"))
        .unwrap();
    blocked.recv_timeout(Duration::from_secs(10)).unwrap();
    let (dropped, returned) = crossbeam_channel::bounded(1);
    let dropper = std::thread::spawn(move || {
        drop(service);
        dropped.send(()).unwrap();
    });
    // A joining destructor would time out here: the IO gate is still closed.
    returned
        .recv_timeout(Duration::from_secs(1))
        .expect("service destructor blocked on disk");
    dropper.join().unwrap();
    assert_eq!(fixture.budget.retained_bytes(), retained);
    assert!(fixture.budget.reserve(Cohort::SavePayload, 1).is_err());
    assert_eq!(
        disk::Store::open(temp.path()).err().unwrap().phase,
        Phase::Open
    );
    release.send(()).unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while fixture.budget.retained_bytes() != complete::RUNNING_AUTHORITY_ALLOWANCE_BYTES {
        assert!(
            Instant::now() < deadline,
            "abandoned worker retained its charge after completion"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
    let store = disk::Store::open(temp.path()).unwrap();
    assert_title(
        &fixture,
        store
            .load(&slot(), LoadSelection::Active, &fixture.budget)
            .unwrap(),
        "finishes after abandonment",
    );
}

#[test]
#[ignore = "explicit bounded release storage timing and retained same-image fixture probe"]
fn storage_release_probe() {
    let root = PathBuf::from(
        std::env::var_os("ALIBI_M3A_PROBE_DIR").expect("explicit isolated probe directory"),
    );
    assert!(root.starts_with("/tmp/"));
    let report_path = PathBuf::from(std::env::var_os("ALIBI_M3A_REPORT").expect("report path"));
    let reader = std::env::var_os("ALIBI_M3A_PROFILE_READ").is_some();
    if !reader {
        std::fs::create_dir(&root).unwrap();
    }
    let fixture = Fixture::new();
    let durable_instants = Arc::new(std::sync::Mutex::new(Vec::with_capacity(32)));
    let observer = Arc::clone(&durable_instants);
    let hook = Arc::new(move |phase, edge| {
        if phase == Phase::PendingSync && edge == disk::Edge::After {
            observer.lock().unwrap().push(Instant::now());
        }
        Ok(())
    });
    let start = Instant::now();
    let service = CheckpointStorage::start_hooked(&root, fixture.budget.clone(), hook).unwrap();
    let startup_us = start.elapsed().as_secs_f64() * 1e6;
    let mut submit = Vec::new();
    let mut attach = Vec::new();
    let mut delivery = Vec::new();
    let mut durable = Vec::new();
    if !reader {
        for sample in 0..32 {
            let start = Instant::now();
            let request = service.request_save(slot()).unwrap();
            submit.push(start.elapsed().as_secs_f64() * 1e6);
            // Existing synchronous capture cost is deliberately outside these
            // backend measurements; no host-frame capture claim is made.
            let payload = fixture.capture();
            let start = Instant::now();
            service
                .attach(
                    request,
                    payload,
                    SaveMetadata::new("A", 1_789_430_400 + sample, "The square").unwrap(),
                )
                .unwrap();
            attach.push(start.elapsed().as_secs_f64() * 1e6);
            let deadline = Instant::now() + Duration::from_secs(20);
            loop {
                let drain = Instant::now();
                if let Some(result) = service.take_result(request) {
                    delivery.push(drain.elapsed().as_secs_f64() * 1e6);
                    saved(result);
                    let actual_durable = durable_instants.lock().unwrap().last().copied().unwrap();
                    durable.push(actual_durable.duration_since(start).as_secs_f64() * 1e6);
                    break;
                }
                assert!(Instant::now() < deadline);
                std::thread::sleep(Duration::from_millis(1));
            }
        }
    }
    let request = service.request_load(slot(), LoadSelection::Active).unwrap();
    let Outcome::Loaded(loaded) = wait_result(&service, request) else {
        panic!("probe load failed");
    };
    let reference = loaded.reference.clone();
    let candidate = fixture.validate(loaded);
    assert_eq!(
        disk::hash(candidate.value().bytes()),
        reference.payload_sha256()
    );
    drop(candidate);
    let start = Instant::now();
    let shutdown = service.shutdown();
    let shutdown_signal_us = start.elapsed().as_secs_f64() * 1e6;
    drop(shutdown.join());
    assert_eq!(
        fixture.budget.retained_bytes(),
        complete::RUNNING_AUTHORITY_ALLOWANCE_BYTES
    );
    let report = serde_json::json!({"schema":1,"mode":if reader {"fresh-reader"} else {"writer"},
        "directory": root, "executable": std::env::current_exe().unwrap(),
        "host_image_sha256": fixture.engine.config().checkpoint_host_image.unwrap(),
        "payload_sha256": reference.payload_sha256(), "payload_bytes": reference.payload_bytes(),
        "generation": reference.generation(), "samples": submit.len(),
        "microseconds":{"startup":startup_us,"submit":submit,"attach":attach,"terminal_take":delivery,"durable_from_attach":durable,"shutdown_signal":shutdown_signal_us},
        "service_allowance_bytes": SERVICE_ALLOWANCE_BYTES,"configured_worker_stack_bytes":2*1024*1024,
        "shared_peak_bytes":fixture.budget.peak_retained_bytes(),"retained_after_shutdown":fixture.budget.retained_bytes(),
        "complete_m2_validation":true,"capture_cost_and_real_host_frames_excluded":true});
    use std::io::Write;
    std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(report_path)
        .unwrap()
        .write_all(&serde_json::to_vec_pretty(&report).unwrap())
        .unwrap();
    println!("{report}");
}
pub(crate) fn metadata(title: &str) -> SaveMetadata {
    SaveMetadata::new(title, 1_789_430_400, "The square").unwrap()
}
pub(crate) fn slot() -> SlotId {
    SlotId::new("manual-1").unwrap()
}
pub(crate) fn wait_result(service: &CheckpointStorage, id: OperationId) -> Outcome {
    let until = Instant::now() + Duration::from_secs(20);
    loop {
        if let Some(result) = service.take_result(id) {
            return result;
        }
        assert!(Instant::now() < until, "storage result deadline");
        std::thread::sleep(Duration::from_millis(1));
    }
}
fn id(sequence: u64) -> OperationId {
    OperationId {
        service: [7; 16],
        sequence,
    }
}
fn saved(result: Outcome) -> SlotReference {
    if let Outcome::Saved(r) = result {
        r
    } else {
        panic!("expected Saved, got {result:?}")
    }
}

#[test]
fn storage_real_complete_save_load_and_two_generation_retention() {
    let fixture = Fixture::new();
    let temp = TempDir::new();
    let service = CheckpointStorage::start(temp.path(), fixture.budget.clone()).unwrap();
    let mut references = Vec::new();
    let mut retained_reader = None;
    for title in ["B", "A", "C", "D"] {
        let op = service.request_save(slot()).unwrap();
        assert_eq!(service.next_capture(), Some(op));
        service
            .attach(op, fixture.capture(), metadata(title))
            .unwrap();
        references.push(saved(wait_result(&service, op)));
        if retained_reader.is_some() {
            // A's admitted input remains alive while C and D are published.
            assert!(fixture.budget.reserve(Cohort::LoadCandidate, 1).is_err());
            continue;
        }
        let load = service.request_load(slot(), LoadSelection::Active).unwrap();
        let Outcome::Loaded(loaded) = wait_result(&service, load) else {
            panic!("not loaded");
        };
        assert_eq!(loaded.reference.metadata().title(), title);
        if title == "A" {
            retained_reader = Some(loaded);
        } else {
            drop(fixture.validate(loaded));
        }
    }
    assert!(
        !temp
            .path()
            .join(disk::payload_file(&slot(), references[1].generation()))
            .exists()
    );
    assert!(
        temp.path()
            .join(disk::payload_file(&slot(), references[2].generation()))
            .exists()
    );
    assert!(
        temp.path()
            .join(disk::payload_file(&slot(), references[3].generation()))
            .exists()
    );
    // Cleanup removed A's immutable filename, but the original admitted bytes
    // and LoadCandidate lease still support full M2 validation independently.
    let retained_reader = retained_reader.unwrap();
    assert_eq!(retained_reader.reference, references[1]);
    assert!(fixture.budget.reserve(Cohort::LoadCandidate, 1).is_err());
    let candidate = fixture.validate(retained_reader);
    assert!(fixture.budget.reserve(Cohort::LoadCandidate, 1).is_err());
    drop(candidate);
    let load = service.request_load(slot(), LoadSelection::Active).unwrap();
    let Outcome::Loaded(loaded) = wait_result(&service, load) else {
        panic!("load after retained reader disposal failed");
    };
    assert_eq!(loaded.reference, references[3]);
    drop(fixture.validate(loaded));
    let done = service.shutdown().join();
    assert!(done.is_finished());
    drop(done);
    assert_eq!(
        fixture.budget.retained_bytes(),
        complete::RUNNING_AUTHORITY_ALLOWANCE_BYTES
    );
}
