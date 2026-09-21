use super::*;
use bounded::{ALLOWANCE_BYTES, RECORD_BYTES, Ticket};
use cathedral_sim::checkpoint::{CheckpointBudget, Cohort};
use std::sync::{
    Arc, Condvar,
    atomic::{AtomicUsize, Ordering},
};
use tracing_subscriber::prelude::*;

#[derive(Default)]
struct Gate {
    state: Mutex<(bool, bool)>,
    changed: Condvar,
}
impl Gate {
    fn enter(&self) {
        let mut s = self.state.lock().unwrap();
        s.0 = true;
        self.changed.notify_all();
        while !s.1 {
            s = self.changed.wait(s).unwrap();
        }
    }
    fn entered(&self) {
        let deadline = Instant::now() + Duration::from_secs(3);
        let mut s = self.state.lock().unwrap();
        while !s.0 {
            let remaining = deadline
                .checked_duration_since(Instant::now())
                .expect("writer never entered gate");
            s = self.changed.wait_timeout(s, remaining).unwrap().0;
        }
    }
    fn release(&self) {
        self.state.lock().unwrap().1 = true;
        self.changed.notify_all();
    }
}
struct SlowWriter {
    gate: Arc<Gate>,
    bytes: Arc<Mutex<Vec<u8>>>,
}
impl Write for SlowWriter {
    fn write(&mut self, b: &[u8]) -> io::Result<usize> {
        self.gate.enter();
        self.bytes.lock().unwrap().extend_from_slice(b);
        Ok(b.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
fn deadline() -> Instant {
    Instant::now() + Duration::from_secs(3)
}

#[test]
fn diagnostic_pool_includes_active_queue_and_formatting_and_preserves_refusal() {
    let gate = Arc::new(Gate::default());
    let bytes = Arc::new(Mutex::new(Vec::new()));
    let owner = BoundedSink::start(
        SlowWriter {
            gate: gate.clone(),
            bytes,
        },
        None,
        2,
        1,
        false,
    )
    .unwrap();
    let sink = owner.sender();
    let first = sink.line_owned("first".into()).unwrap();
    gate.entered();
    let draft = sink.begin(false, None).unwrap();
    assert_eq!((sink.usage().active, sink.usage().formatting), (1, 1));
    let mut original = String::with_capacity(99);
    original.push_str("preserve exactly");
    let pointer = original.as_ptr();
    let capacity = original.capacity();
    let (why, returned) = sink.line_owned(original).unwrap_err();
    assert_eq!(why, Refusal::Full);
    assert_eq!(returned.as_ptr(), pointer);
    assert_eq!(returned.capacity(), capacity);
    drop(draft);
    sink.line_owned("second".into()).unwrap();
    let evidence =
        format::line(&sink, "drive", "INFO", "protected", true, Some(deadline())).unwrap();
    assert_eq!((sink.usage().active, sink.usage().queued), (1, 2));
    assert_eq!(
        sink.fence(Some(evidence), Instant::now() + Duration::from_millis(5)),
        Err(Refusal::Timeout)
    );
    gate.release();
    sink.fence(Some(evidence), deadline()).unwrap();
    sink.fence(Some(first), deadline()).unwrap();
    assert_eq!(sink.usage().completed, evidence.0);
}

#[derive(Clone)]
struct Output(Arc<Mutex<Vec<u8>>>);
impl Write for Output {
    fn write(&mut self, b: &[u8]) -> io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(b);
        Ok(b.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
#[test]
fn jsonl_refuses_whole_escaped_record_and_preserves_unicode_and_fields() {
    let bytes = Arc::new(Mutex::new(Vec::new()));
    let owner = BoundedSink::start(Output(bytes.clone()), None, 2, 1, false).unwrap();
    let sink = owner.sender();
    assert_eq!(
        format::line(
            &sink,
            "engine",
            "INFO",
            &"\u{0}".repeat(RECORD_BYTES / 2),
            false,
            None
        ),
        Err(Refusal::Oversized)
    );
    let ticket = format::line(&sink, "engine", "INFO", "Å\n中\"e\u{301}", false, None).unwrap();
    sink.fence(Some(ticket), deadline()).unwrap();
    let log = bytes.lock().unwrap();
    let rows: Vec<serde_json::Value> = log
        .split(|b| *b == b'\n')
        .filter(|b| !b.is_empty())
        .map(|b| serde_json::from_slice(b).unwrap())
        .collect();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["message"], "Å\n中\"e\u{301}");
    assert_eq!(sink.usage().refused_oversized, 1);
}

struct EventLayer(SinkSender);
impl<S: tracing::Subscriber> tracing_subscriber::Layer<S> for EventLayer {
    fn on_event(&self, event: &tracing::Event<'_>, _: tracing_subscriber::layer::Context<'_, S>) {
        let _ = format::event(&self.0, event, true);
    }
}
struct HugeDebug(Arc<AtomicUsize>);
impl std::fmt::Debug for HugeDebug {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for _ in 0..1_000_000 {
            self.0.fetch_add(1, Ordering::Relaxed);
            f.write_str("1234567890")?;
        }
        Ok(())
    }
}
#[test]
fn tracing_debug_is_bounded_before_owned_copy_and_typed_fields_survive() {
    let bytes = Arc::new(Mutex::new(Vec::new()));
    let owner = BoundedSink::start(Output(bytes.clone()), None, 2, 1, false).unwrap();
    let sink = owner.sender();
    let calls = Arc::new(AtomicUsize::new(0));
    tracing::subscriber::with_default(
        tracing_subscriber::registry().with(EventLayer(sink.clone())),
        || {
            tracing::info!(huge = ?HugeDebug(calls.clone()), "discard this whole event");
            tracing::info!(answer = 42_u64, valid = true, text = "中", "hello Å");
        },
    );
    sink.fence(None, deadline()).unwrap();
    assert!(calls.load(Ordering::Relaxed) < 4000);
    let log = bytes.lock().unwrap();
    let row: serde_json::Value = serde_json::from_slice(&log).unwrap();
    assert_eq!(row["message"], "hello Å");
    assert_eq!(row["fields"]["answer"], 42);
    assert_eq!(row["fields"]["valid"], true);
    assert_eq!(row["fields"]["text"], "中");
    let oversized = String::with_capacity(RECORD_BYTES * 8);
    let ptr = oversized.as_ptr();
    let (why, value) = sink.line_owned(oversized).unwrap_err();
    assert_eq!(why, Refusal::Oversized);
    assert_eq!(value.as_ptr(), ptr);
    assert_eq!(value.capacity(), RECORD_BYTES * 8);
}

struct ExitGate(Arc<Gate>);
impl Drop for ExitGate {
    fn drop(&mut self) {
        self.0.enter();
    }
}
thread_local! { static EXIT_GATE: std::cell::RefCell<Option<ExitGate>> = const { std::cell::RefCell::new(None) }; }
struct TlsWriter(Arc<Gate>);
impl Write for TlsWriter {
    fn write(&mut self, b: &[u8]) -> io::Result<usize> {
        EXIT_GATE.with(|g| *g.borrow_mut() = Some(ExitGate(self.0.clone())));
        Ok(b.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
#[test]
fn diagnostic_allowance_survives_final_native_tls_termination() {
    let budget = CheckpointBudget::default();
    let root = budget.reserve(Cohort::Running, 512 * 1024 * 1024).unwrap();
    let gate = Arc::new(Gate::default());
    let owner = BoundedSink::start_admitted(TlsWriter(gate.clone()), &budget).unwrap();
    let sink = owner.sender();
    let t = sink.line_owned("one write".into()).unwrap();
    sink.fence(Some(t), deadline()).unwrap();
    drop(sink);
    let joining = std::thread::spawn(move || drop(owner));
    gate.entered();
    assert_eq!(budget.retained_bytes(), root.bytes() + ALLOWANCE_BYTES);
    assert!(!joining.is_finished());
    gate.release();
    joining.join().unwrap();
    assert_eq!(budget.retained_bytes(), root.bytes());
}

#[test]
fn joined_worker_does_not_release_surviving_endpoint_or_formatting_storage() {
    let budget = CheckpointBudget::default();
    let root = budget.reserve(Cohort::Running, 512 * 1024 * 1024).unwrap();
    let owner = BoundedSink::start_admitted(io::sink(), &budget).unwrap();
    let sink = owner.sender();
    let draft = sink.begin(false, None).unwrap();
    drop(owner);
    assert_eq!(budget.retained_bytes(), root.bytes() + ALLOWANCE_BYTES);
    assert_eq!(sink.commit(draft, false), Err(Refusal::Closed));
    assert_eq!(budget.retained_bytes(), root.bytes() + ALLOWANCE_BYTES);
    drop(sink);
    assert_eq!(budget.retained_bytes(), root.bytes());
}

#[test]
fn failed_startup_releases_only_after_owned_writer_disposal() {
    struct DropWriter(Arc<Gate>);
    impl Write for DropWriter {
        fn write(&mut self, b: &[u8]) -> io::Result<usize> {
            Ok(b.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    impl Drop for DropWriter {
        fn drop(&mut self) {
            self.0.enter();
        }
    }
    let budget = CheckpointBudget::default();
    let root = budget.reserve(Cohort::Running, 512 * 1024 * 1024).unwrap();
    let lease = budget.reserve_running_overhead(ALLOWANCE_BYTES).unwrap();
    let gate = Arc::new(Gate::default());
    let other = gate.clone();
    let startup = std::thread::spawn(move || {
        assert!(BoundedSink::start(DropWriter(other), Some(lease), 1, 1, true).is_err())
    });
    gate.entered();
    assert_eq!(budget.retained_bytes(), root.bytes() + ALLOWANCE_BYTES);
    gate.release();
    startup.join().unwrap();
    assert_eq!(budget.retained_bytes(), root.bytes());
}

#[test]
fn failed_write_has_no_successful_fence_and_no_unbounded_retry() {
    struct Fail;
    impl Write for Fail {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            Err(io::Error::other("controlled failure"))
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    let owner = BoundedSink::start(Fail, None, 1, 1, false).unwrap();
    let sink = owner.sender();
    let ticket = sink.line_owned("cannot write".into()).unwrap();
    assert_eq!(sink.fence(Some(ticket), deadline()), Err(Refusal::Io));
    assert!(sink.usage().io_failed);
    assert_eq!(
        sink.fence(Some(Ticket(ticket.0 + 1)), deadline()),
        Err(Refusal::Closed)
    );
}
