//! Independent M3b2b2 witnesses for real native work and its retained owners.
use crate::{
    BackendRuntime, backend_channel_for,
    config::{BackendsConfig, BackendsOptions, Environment},
    runtime::RUNTIME_ALLOWANCE_BYTES,
    transcription::{DISCARD_QUEUE_CAPACITY, MAX_RECORDING_PATH_BYTES, SttEngine},
};
use cathedral_sim::{
    RuntimeGeneration,
    checkpoint::{
        CheckpointBudget, Cohort, RetirementLease, complete::RUNNING_AUTHORITY_ALLOWANCE_BYTES,
    },
};
use std::{
    collections::BTreeMap,
    io,
    net::SocketAddr,
    path::PathBuf,
    sync::{Arc, Condvar, Mutex, mpsc},
    time::Duration,
};

const RETIREMENT_BYTES: usize = 8 * 1024 * 1024;
const WAIT: Duration = Duration::from_secs(20);
type GateState = Arc<(Mutex<bool>, Condvar)>;

/// Opening on unwind prevents a failed assertion from stranding a native join.
struct OpenOnDrop(GateState);
impl OpenOnDrop {
    fn new() -> Self {
        Self(Arc::new((Mutex::new(false), Condvar::new())))
    }
    fn open(&self) {
        *self.0.0.lock().unwrap() = true;
        self.0.1.notify_all();
    }
}
impl Drop for OpenOnDrop {
    fn drop(&mut self) {
        self.open();
    }
}
fn wait_for_gate(gate: &GateState) {
    let (opened, timeout) = gate
        .1
        .wait_timeout_while(gate.0.lock().unwrap(), WAIT, |open| !*open)
        .unwrap();
    assert!(
        *opened && !timeout.timed_out(),
        "test release gate timed out"
    );
}
fn retirement(budget: &CheckpointBudget) -> RetirementLease {
    RetirementLease::new(
        budget
            .reserve(Cohort::RetiringGeneration, RETIREMENT_BYTES)
            .unwrap(),
    )
    .unwrap()
}
fn address() -> SocketAddr {
    "127.0.0.1:80".parse().unwrap()
}

#[test]
fn native_review_dns_answer_and_executor_keep_separate_charges_after_runtime_join() {
    let budget = CheckpointBudget::default();
    let running = budget
        .reserve(Cohort::Running, RUNNING_AUTHORITY_ALLOWANCE_BYTES)
        .unwrap();
    let runtime = BackendRuntime::start_admitted(&budget).unwrap();
    let executor = runtime.executor();
    let pin = retirement(&budget);
    let released = pin.release_observer();
    let (events, receiver) = backend_channel_for(RuntimeGeneration(501));
    events.pin_retirement(&budget, &pin).unwrap();
    let resolver = runtime.resolver(events.clone());
    let mut answer = runtime
        .block_on(resolver.run("controlled-answer".into(), |_| Ok(vec![address()])))
        .unwrap();
    assert_eq!(answer.next(), Some(address()));
    assert_eq!(answer.next(), None);
    receiver.fence();
    drop((resolver, events, receiver, pin));
    drop(runtime); // Actual native runtime join; the exhausted iterator survives.
    assert!(!released.released());
    assert_eq!(
        budget.retained_bytes(),
        RUNNING_AUTHORITY_ALLOWANCE_BYTES + RUNTIME_ALLOWANCE_BYTES + RETIREMENT_BYTES
    );
    assert!(budget.reserve(Cohort::RetiringGeneration, 1).is_err());
    drop(answer);
    assert!(released.released());
    assert_eq!(
        budget.retained_bytes(),
        RUNNING_AUTHORITY_ALLOWANCE_BYTES + RUNTIME_ALLOWANCE_BYTES
    );
    drop(executor);
    assert_eq!(budget.retained_bytes(), RUNNING_AUTHORITY_ALLOWANCE_BYTES);
    assert_eq!(
        budget.cohort_bytes(Cohort::Running),
        RUNNING_AUTHORITY_ALLOWANCE_BYTES
    );
    drop(running);
    assert_eq!(budget.retained_bytes(), 0);
}

#[test]
fn native_review_cancelled_dns_waiters_keep_slots_and_charge_until_off_frame_join() {
    let budget = CheckpointBudget::default();
    let running = budget
        .reserve(Cohort::Running, RUNNING_AUTHORITY_ALLOWANCE_BYTES)
        .unwrap();
    let runtime = BackendRuntime::start_admitted(&budget).unwrap();
    let gate = OpenOnDrop::new();
    let pin = retirement(&budget);
    let released = pin.release_observer();
    let (events, receiver) = backend_channel_for(RuntimeGeneration(511));
    events.pin_retirement(&budget, &pin).unwrap();
    let resolver = runtime.resolver(events.clone());
    let (started, starts) = mpsc::sync_channel(2);
    let mut waiters = Vec::new();
    for index in 0..2 {
        let resolver = resolver.clone();
        let gate = Arc::clone(&gate.0);
        let started = started.clone();
        waiters.push(runtime.spawn(async move {
            resolver
                .run(format!("controlled-{index}"), move |_| {
                    started.send(()).unwrap();
                    wait_for_gate(&gate);
                    Ok(vec![address()])
                })
                .await
                .map(|answer| answer.count())
        }));
    }
    for _ in 0..2 {
        starts.recv_timeout(WAIT).unwrap();
    }
    for waiter in waiters {
        waiter.abort();
        assert!(runtime.block_on(waiter).unwrap_err().is_cancelled());
    }
    let refused = runtime.block_on(resolver.run("refused".into(), |_| {
        panic!("cancelled waiters must not free native DNS admission")
    }));
    assert!(matches!(refused, Err(ref error) if error.kind() == io::ErrorKind::WouldBlock));
    receiver.fence();
    drop((resolver, events, receiver, pin));
    assert!(!released.released());

    let host = std::thread::current().id();
    let (dropping, drop_started) = mpsc::sync_channel(1);
    let (done, completion) = mpsc::sync_channel(1);
    let retirement_worker = std::thread::spawn(move || {
        assert_ne!(std::thread::current().id(), host);
        dropping.send(()).unwrap();
        drop(runtime);
        let _ = done.send(());
    });
    drop_started.recv_timeout(WAIT).unwrap();
    // This is a blocked native-work witness, not a frame-latency measurement.
    assert!(matches!(
        completion.recv_timeout(Duration::from_millis(50)),
        Err(mpsc::RecvTimeoutError::Timeout)
    ));
    assert!(!released.released());
    assert_eq!(
        budget.retained_bytes(),
        RUNNING_AUTHORITY_ALLOWANCE_BYTES + RUNTIME_ALLOWANCE_BYTES + RETIREMENT_BYTES
    );
    gate.open();
    completion.recv_timeout(WAIT).unwrap();
    retirement_worker.join().unwrap();
    assert!(released.released());
    assert_eq!(budget.retained_bytes(), RUNNING_AUTHORITY_ALLOWANCE_BYTES);
    assert_eq!(
        budget.cohort_bytes(Cohort::Running),
        RUNNING_AUTHORITY_ALLOWANCE_BYTES
    );
    drop(running);
    assert_eq!(budget.retained_bytes(), 0);
}

struct TemporaryRecordings(PathBuf);
impl Drop for TemporaryRecordings {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn native_review_recording_disposal_retains_full_queue_and_generation_through_join() {
    let directory = TemporaryRecordings(std::env::temp_dir().join(format!(
        "alibi-native-retention-review-{}-{}",
        std::process::id(),
        crate::session_dir::SessionDir::new_session_id()
    )));
    std::fs::create_dir(&directory.0).unwrap();
    let settings = BackendsConfig::resolve(
        &Environment::from_map(BTreeMap::new()),
        &BackendsOptions {
            dotenv_path: None,
            workers_dir: directory.0.join("absent-workers"),
            uv_binary: "uv".into(),
            fake_mode: false,
        },
    )
    .speech;
    assert!(settings.api_key.is_none());
    let budget = CheckpointBudget::default();
    let running = budget
        .reserve(Cohort::Running, RUNNING_AUTHORITY_ALLOWANCE_BYTES)
        .unwrap();
    let runtime = BackendRuntime::start_admitted(&budget).unwrap();
    let pin = retirement(&budget);
    let released = pin.release_observer();
    let (events, receiver) = backend_channel_for(RuntimeGeneration(521));
    events.pin_retirement(&budget, &pin).unwrap();
    let gate = OpenOnDrop::new();
    let worker_gate = Arc::clone(&gate.0);
    let (started, starts) = mpsc::sync_channel(DISCARD_QUEUE_CAPACITY + 1);
    let stt = SttEngine::with_discard_hook(
        runtime,
        &settings,
        Some(directory.0.clone()),
        events.clone(),
        Arc::new(move |_| {
            started.send(()).unwrap();
            wait_for_gate(&worker_gate);
        }),
    );
    // Declared after SttEngine so an assertion failure opens before its Drop joins.
    let _unwind_release = OpenOnDrop(Arc::clone(&gate.0));
    let mut recordings = Vec::new();
    for index in 0..=DISCARD_QUEUE_CAPACITY {
        let path = directory.0.join(format!("recording-{index}.wav"));
        std::fs::write(&path, b"controlled recording").unwrap();
        stt.try_discard_resolved(path.clone()).unwrap();
        recordings.push(path);
        if index == 0 {
            starts.recv_timeout(WAIT).unwrap(); // One active, then exactly 64 queued.
        }
    }
    let mut refused = PathBuf::with_capacity(256);
    refused.push("full-queue.wav");
    let pointer = refused.as_os_str().as_encoded_bytes().as_ptr();
    let capacity = refused.capacity();
    let refused = stt.try_discard_resolved(refused).unwrap_err();
    assert_eq!(refused.as_os_str().as_encoded_bytes().as_ptr(), pointer);
    assert_eq!(refused.capacity(), capacity);
    assert_eq!(refused, PathBuf::from("full-queue.wav"));
    let mut oversized = PathBuf::with_capacity(MAX_RECORDING_PATH_BYTES + 1);
    oversized.push("tiny.wav");
    let pointer = oversized.as_os_str().as_encoded_bytes().as_ptr();
    let oversized = stt.try_discard_resolved(oversized).unwrap_err();
    assert_eq!(oversized.as_os_str().as_encoded_bytes().as_ptr(), pointer);
    assert!(recordings.iter().all(|path| path.exists()));
    receiver.fence();
    assert!(
        stt.try_discard_resolved(PathBuf::from("fenced.wav"))
            .is_err()
    );
    drop((events, receiver, pin));
    let host = std::thread::current().id();
    let (dropping, drop_started) = mpsc::sync_channel(1);
    let (done, completion) = mpsc::sync_channel(1);
    let retirement_worker = std::thread::spawn(move || {
        assert_ne!(std::thread::current().id(), host);
        dropping.send(()).unwrap();
        drop(stt);
        let _ = done.send(());
    });
    drop_started.recv_timeout(WAIT).unwrap();
    assert!(matches!(
        completion.recv_timeout(Duration::from_millis(50)),
        Err(mpsc::RecvTimeoutError::Timeout)
    ));
    assert!(!released.released());
    assert_eq!(
        budget.retained_bytes(),
        RUNNING_AUTHORITY_ALLOWANCE_BYTES + RUNTIME_ALLOWANCE_BYTES + RETIREMENT_BYTES
    );
    assert!(budget.reserve(Cohort::RetiringGeneration, 1).is_err());
    gate.open();
    completion.recv_timeout(WAIT).unwrap();
    retirement_worker.join().unwrap();
    assert!(recordings.iter().all(|path| !path.exists()));
    assert!(released.released());
    assert_eq!(budget.retained_bytes(), RUNNING_AUTHORITY_ALLOWANCE_BYTES);
    assert_eq!(
        budget.cohort_bytes(Cohort::Running),
        RUNNING_AUTHORITY_ALLOWANCE_BYTES
    );
    drop(running);
    assert_eq!(budget.retained_bytes(), 0);
}
