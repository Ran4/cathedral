//! Independent ownership checks against the production archive worker.
use crate::prompt_log::{
    ARCHIVE_ALLOWANCE_BYTES, ArchiveWriter, LocalTime, PromptExchange, PromptLog,
};
use cathedral_sim::{
    ActorId, Cognition, CognitionBusy, RequestId,
    checkpoint::{CheckpointBudget, Cohort, complete::RUNNING_AUTHORITY_ALLOWANCE_BYTES},
    prompt_archive::{PromptExchange as SharedExchange, PromptExchangeData},
};
use std::{
    fs,
    path::PathBuf,
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let path = std::env::temp_dir().join(format!(
            "alibi-m3b2b1-review-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

struct Gate {
    state: Arc<(Mutex<bool>, Condvar)>,
    entered: crossbeam_channel::Receiver<()>,
}
impl Gate {
    fn new() -> (Self, Arc<dyn Fn() + Send + Sync>) {
        let state = Arc::new((Mutex::new(false), Condvar::new()));
        let worker = Arc::clone(&state);
        let (entered, seen) = crossbeam_channel::bounded(8);
        let hook = Arc::new(move || {
            let _ = entered.try_send(());
            let (lock, wake) = &*worker;
            let mut open = lock.lock().unwrap();
            while !*open {
                open = wake.wait(open).unwrap();
            }
        });
        (
            Self {
                state,
                entered: seen,
            },
            hook,
        )
    }
    fn wait(&self) {
        self.entered.recv_timeout(Duration::from_secs(20)).unwrap();
    }
    fn open(&self) {
        *self.state.0.lock().unwrap() = true;
        self.state.1.notify_all();
    }
}
impl Drop for Gate {
    fn drop(&mut self) {
        self.open();
    }
}

// Unwind opens the gate before PromptLog::drop flushes; only then is the
// test-owned directory removed. A failed assertion cannot strand this worker.
struct Harness {
    gate: Gate,
    log: PromptLog,
    writer: ArchiveWriter,
    directory: Directory,
}
fn moment() -> LocalTime {
    LocalTime {
        year: 2026,
        month: 9,
        day: 15,
        hour: 8,
        minute: 0,
        second: 0,
    }
}
impl Harness {
    fn new(slots: usize, bytes: usize) -> Self {
        let directory = Directory::new();
        let (gate, hook) = Gate::new();
        let writer = ArchiveWriter::isolated(slots, bytes, hook);
        let log = PromptLog::with_writer(
            Some(directory.0.clone()),
            Some("review".into()),
            Box::new(moment),
            writer.clone(),
        );
        Self {
            gate,
            log,
            writer,
            directory,
        }
    }
    fn archives(&self) -> Vec<(String, serde_json::Value)> {
        self.log.flush();
        let mut records: Vec<_> = fs::read_dir(&self.directory.0)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| {
                path.extension()
                    .is_some_and(|extension| extension == "json")
            })
            .map(|path| {
                let name = path.file_name().unwrap().to_str().unwrap().to_owned();
                (
                    name,
                    serde_json::from_slice(&fs::read(path).unwrap()).unwrap(),
                )
            })
            .collect();
        records.sort_by(|a, b| a.0.cmp(&b.0));
        records
    }
}
fn input(prompt: &str, capacity: usize) -> PromptExchange {
    let mut text = String::with_capacity(capacity);
    text.push_str(prompt);
    PromptExchange {
        actor_id: "review-actor".into(),
        actor_name: "Review Actor".into(),
        prompt: text,
        answer: Some("wait {}".into()),
        duration_seconds: 0.125,
        error: None,
    }
}
struct Unavailable;
impl Cognition for Unavailable {
    fn request(&mut self, _: String) -> Result<RequestId, CognitionBusy> {
        Err(CognitionBusy)
    }
}

#[test]
fn archive_review_shared_actor_id_spare_capacity_cannot_bypass_admission() {
    for preadmitted in [false, true] {
        let mut h = Harness::new(2, 2 * 1024 * 1024);
        let mut id = String::with_capacity(1024 * 1024);
        id.push_str("review-actor");
        let permit = if preadmitted {
            h.log
                .cognition(Box::new(Unavailable))
                .reserve_prompt_archive(16, 64)
                .unwrap()
        } else {
            Default::default()
        };
        let exchange = SharedExchange::new(
            PromptExchangeData {
                actor_id: ActorId::from_raw(id),
                actor_name: "Review Actor".into(),
                prompt: "small text".into(),
                answer: None,
                duration_seconds: 0.0,
                error: None,
            },
            permit,
        );
        let pointer = Arc::as_ptr(&exchange);
        let before = h.writer.usage();
        let returned = h.log.record_shared(exchange).unwrap_err();
        assert_eq!(Arc::as_ptr(&returned), pointer);
        assert_eq!(returned.actor_id.allocated_bytes(), 1024 * 1024);
        assert_eq!(h.writer.usage(), before);
        drop(returned);
        assert_eq!(h.writer.usage().slots, 0);
    }
}

#[test]
fn archive_review_unadmitted_shared_input_refuses_and_convenience_owns_a_separate_copy() {
    let mut h = Harness::new(1, 2 * 1024 * 1024);
    let original = SharedExchange::new(
        PromptExchangeData {
            actor_id: ActorId::from_raw("review-actor"),
            actor_name: "Review Actor".into(),
            prompt: "caller-owned source survives the archive".into(),
            answer: None,
            duration_seconds: 0.0,
            error: None,
        },
        Default::default(),
    );
    let pointer = original.prompt.as_ptr();
    let refused = h.log.record_shared(Arc::clone(&original)).unwrap_err();
    assert!(Arc::ptr_eq(&original, &refused));
    assert_eq!(h.writer.usage().slots, 0);
    drop(refused);
    let event = cathedral_sim::SchedulerEvent::PromptExchange {
        exchange: Arc::clone(&original),
    };
    h.log.record_scheduler_event(&event).unwrap();
    h.gate.wait();
    assert_eq!(h.writer.usage().slots, 1);
    assert!(!original.admission().enabled());
    assert_eq!(original.prompt.as_ptr(), pointer);
    h.gate.open();
    h.log.flush();
    assert_eq!(h.writer.usage().slots, 0);
    assert_eq!(h.writer.usage().bytes, 0);
    assert_eq!(original.prompt, "caller-owned source survives the archive");
    let records = h.archives();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].1["prompt"], original.prompt);
}
fn shared(log: &PromptLog, text: &str) -> Arc<SharedExchange> {
    let mut cognition = log.cognition(Box::new(Unavailable));
    let prompt = text.to_owned();
    let permit = cognition
        .reserve_prompt_archive(prompt.capacity(), 64)
        .unwrap();
    SharedExchange::new(
        PromptExchangeData {
            actor_id: ActorId::from_raw("review-actor"),
            actor_name: "Review Actor".into(),
            prompt,
            answer: Some("wait {}".into()),
            duration_seconds: 0.125,
            error: None,
        },
        permit,
    )
}

#[test]
fn archive_review_byte_refusal_returns_the_same_input_without_spending_a_filename() {
    let mut h = Harness::new(8, 20 * 1024);
    h.log.record(input("first", 4096)).unwrap();
    h.gate.wait();
    let before = h.writer.usage();
    assert_eq!((before.slots, before.active), (1, 1));
    let second = input("second", 4096);
    let pointer = second.prompt.as_ptr();
    let capacity = second.prompt.capacity();
    let returned = h.log.record(second).unwrap_err();
    assert_eq!(returned.prompt.as_ptr(), pointer);
    assert_eq!(returned.prompt.capacity(), capacity);
    assert_eq!(returned.prompt, "second");
    assert_eq!(h.writer.usage(), before);
    h.gate.open();
    h.log.flush();
    h.log.record(returned).unwrap();
    let records = h.archives();
    assert_eq!(records.len(), 2);
    assert!(
        records[0]
            .0
            .contains("__00__review-actor__Review-Actor_prompt.json")
    );
    assert!(
        records[1]
            .0
            .contains("__01__review-actor__Review-Actor_prompt.json")
    );
    assert_eq!(records[0].1["prompt"], "first");
    assert_eq!(records[1].1["prompt"], "second");
    assert_eq!(h.writer.usage().slots, 0);
}

#[test]
fn archive_review_event_clones_keep_capacity_after_the_writer_finishes() {
    let mut h = Harness::new(1, 2 * 1024 * 1024);
    let exchange = shared(&h.log, "retained by a presentation consumer");
    let retained = Arc::clone(&exchange);
    let original_bytes = h.writer.usage().bytes;
    h.log.record_shared(exchange).unwrap();
    h.gate.wait();
    let refused = h.log.record_shared(Arc::clone(&retained)).unwrap_err();
    assert!(Arc::ptr_eq(&refused, &retained));
    drop(refused);
    h.gate.open();
    h.log.flush();
    let after_write = h.writer.usage();
    assert_eq!(
        (after_write.slots, after_write.bytes, after_write.active),
        (1, original_bytes, 0)
    );
    assert!(
        h.log
            .cognition(Box::new(Unavailable))
            .reserve_prompt_archive(1, 1)
            .is_err()
    );
    assert_eq!(h.archives().len(), 1);
    drop(retained);
    assert_eq!(h.writer.usage().slots, 0);
    assert_eq!(h.writer.usage().bytes, 0);
}

#[test]
fn archive_review_forks_preserve_order_and_foreign_sessions_return_the_same_arc() {
    let mut h = Harness::new(8, 4 * 1024 * 1024);
    h.gate.open();
    let mut successor = h.log.fork_with_clock(Box::new(moment));
    h.log.record(input("old session handle", 32)).unwrap();
    successor.record(input("replacement handle", 32)).unwrap();
    h.log.record(input("old handle finishing", 32)).unwrap();
    let exchange = shared(&h.log, "same session only");
    let mut foreign = PromptLog::with_writer(
        Some(h.directory.0.clone()),
        Some("review".into()),
        Box::new(moment),
        h.writer.clone(),
    );
    let returned = foreign.record_shared(Arc::clone(&exchange)).unwrap_err();
    assert!(Arc::ptr_eq(&returned, &exchange));
    drop(exchange);
    successor.record_shared(returned).unwrap();
    successor.flush();
    let records = h.archives();
    assert_eq!(records.len(), 4);
    for (index, (name, _)) in records.iter().enumerate() {
        assert!(
            name.contains(&format!("__{index:02}__review-actor__")),
            "{name}"
        );
    }
    assert_eq!(
        records
            .iter()
            .map(|(_, value)| value["prompt"].as_str().unwrap())
            .collect::<Vec<_>>(),
        [
            "old session handle",
            "replacement handle",
            "old handle finishing",
            "same session only"
        ]
    );
}

#[test]
fn archive_review_actual_admitted_worker_and_permit_keep_the_persistent_charge() {
    let budget = CheckpointBudget::default();
    let running = budget
        .reserve(Cohort::Running, RUNNING_AUTHORITY_ALLOWANCE_BYTES)
        .unwrap();
    let writer = ArchiveWriter::start_admitted(&budget).unwrap();
    let directory = Directory::new();
    let log = PromptLog::with_writer(
        Some(directory.0.clone()),
        None,
        Box::new(moment),
        writer.clone(),
    );
    let mut cognition = log.cognition(Box::new(Unavailable));
    let permit = cognition.reserve_prompt_archive(128, 64).unwrap();
    drop(log);
    drop(writer);
    drop(cognition);
    assert_eq!(
        budget.cohort_bytes(Cohort::Running),
        RUNNING_AUTHORITY_ALLOWANCE_BYTES + ARCHIVE_ALLOWANCE_BYTES
    );
    drop(permit);
    let deadline = Instant::now() + Duration::from_secs(20);
    while budget.retained_bytes() != RUNNING_AUTHORITY_ALLOWANCE_BYTES {
        assert!(
            Instant::now() < deadline,
            "the actual worker retained its charge after shutdown"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
    drop(running);
    assert_eq!(budget.retained_bytes(), 0);
}
