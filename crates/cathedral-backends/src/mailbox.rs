//! Bounded, generation-owned delivery. A submission reserves its terminal
//! record before it becomes accepted work. That reservation survives worker
//! completion until the host consumes the queued result.

use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;

use cathedral_sim::{RuntimeEnvelope, RuntimeGeneration};
use crossbeam_channel::{Receiver, RecvTimeoutError, Sender, TryRecvError, bounded};

use crate::events::BackendEvent;

pub const CALLBACK_CAPACITY: usize = 256;
pub const TERMINAL_CAPACITY: usize = 64;
pub const TERMINAL_BYTE_CAPACITY: usize = 64 * 1024 * 1024;
pub const STREAM_BYTE_CAPACITY: usize = 4 * 1024 * 1024;
pub const CALLBACK_DRAIN_CAPACITY: usize = 128;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct MailboxUsage {
    pub records: usize,
    pub terminals: usize,
    pub terminal_bytes: usize,
    pub stream_bytes: usize,
}

#[derive(Debug)]
struct Shared {
    generation: RuntimeGeneration,
    active: AtomicBool,
    usage: Mutex<MailboxUsage>,
    retirement: tokio::sync::Notify,
    #[cfg(test)]
    chunk_latch: Mutex<Option<(Sender<()>, Receiver<()>)>>,
}

#[derive(Debug)]
struct Charge {
    shared: Arc<Shared>,
    bytes: usize,
    terminal: bool,
}

impl Drop for Charge {
    fn drop(&mut self) {
        let mut usage = self.shared.usage.lock().unwrap_or_else(|e| e.into_inner());
        usage.records -= 1;
        if self.terminal {
            usage.terminals -= 1;
            usage.terminal_bytes -= self.bytes;
        } else {
            usage.stream_bytes -= self.bytes;
        }
    }
}

#[derive(Debug)]
struct Queued {
    event: BackendEvent,
    _charge: Charge,
}

#[derive(Debug)]
struct Lease {
    sender: Sender<Queued>,
    shared: Arc<Shared>,
    charge: Mutex<Option<Charge>>,
    fallback: BackendEvent,
    failed: AtomicBool,
}

impl Lease {
    fn publish(&self, mut event: BackendEvent) {
        let Some(charge) = self.charge.lock().unwrap_or_else(|e| e.into_inner()).take() else {
            return; // late end, chunk, panic or duplicate: terminal already owned
        };
        if !self.shared.active.load(Ordering::Acquire) {
            return; // deliberate retirement cancels the whole old generation
        }
        if self.failed.load(Ordering::Acquire) || event.retained_bytes() > charge.bytes {
            event = self.fallback.clone();
        }
        // Every queued record owns one slot. This reserved slot cannot be
        // consumed by status/PCM traffic or by another accepted job.
        let _ = self.sender.try_send(Queued {
            event,
            _charge: charge,
        });
    }
}

impl Drop for Lease {
    fn drop(&mut self) {
        // Unexpected task/worker drop produces one explicit failure. Retired
        // generation drops intentionally produce no new-world callback.
        self.publish(self.fallback.clone());
    }
}

/// Immutable binding cloned into workers. A job clone additionally owns a
/// shared terminal lease; the last clone reports unexpected producer loss.
#[derive(Debug, Clone)]
pub struct BackendSender {
    sender: Sender<Queued>,
    shared: Arc<Shared>,
    lease: Option<Arc<Lease>>,
}

impl BackendSender {
    pub fn generation(&self) -> RuntimeGeneration {
        self.shared.generation
    }

    pub fn is_active(&self) -> bool {
        self.shared.active.load(Ordering::Acquire)
    }

    pub async fn retired(&self) {
        let notified = self.shared.retirement.notified();
        tokio::pin!(notified);
        notified.as_mut().enable();
        if self.is_active() {
            notified.await;
        }
    }

    /// Admission is nonblocking. The fallback identifies precisely the job
    /// whose accepted result must survive saturation or producer failure.
    pub fn reserve(&self, fallback: BackendEvent) -> Option<Self> {
        if self.lease.is_some() || !self.is_active() || !fallback.is_terminal() {
            return None;
        }
        // Preflight borrowed machine identities before cancellation clones them.
        let identity_bytes = match &fallback {
            BackendEvent::TtsDone { event_id, .. }
            | BackendEvent::TtsStreamEnd { event_id, .. } => event_id.0.capacity(),
            BackendEvent::RealtimeResult(cathedral_sim::RealtimeResult::Transcript {
                key, ..
            }) => key.capacity(),
            BackendEvent::RealtimeResult(cathedral_sim::RealtimeResult::Failure {
                key, ..
            }) => key.as_ref().map_or(0, String::capacity),
            _ => 0,
        };
        if identity_bytes > 256 {
            return None;
        }
        let fallback = fallback.cancellation();
        let bytes = fallback.terminal_budget();
        if fallback.retained_bytes() > 4096 {
            return None;
        }
        let charge = self.charge(bytes, true)?;
        Some(Self {
            sender: self.sender.clone(),
            shared: Arc::clone(&self.shared),
            lease: Some(Arc::new(Lease {
                sender: self.sender.clone(),
                shared: Arc::clone(&self.shared),
                charge: Mutex::new(Some(charge)),
                fallback,
                failed: AtomicBool::new(false),
            })),
        })
    }

    fn charge(&self, bytes: usize, terminal: bool) -> Option<Charge> {
        let mut usage = self.shared.usage.lock().unwrap_or_else(|e| e.into_inner());
        if !self.is_active() || usage.records == CALLBACK_CAPACITY {
            return None;
        }
        if terminal {
            if usage.terminals == TERMINAL_CAPACITY
                || bytes > TERMINAL_BYTE_CAPACITY.saturating_sub(usage.terminal_bytes)
            {
                return None;
            }
            usage.terminals += 1;
            usage.terminal_bytes += bytes;
        } else {
            if bytes > STREAM_BYTE_CAPACITY.saturating_sub(usage.stream_bytes) {
                return None;
            }
            usage.stream_bytes += bytes;
        }
        usage.records += 1;
        Some(Charge {
            shared: Arc::clone(&self.shared),
            bytes,
            terminal,
        })
    }

    /// Lifecycle status is bounded, disposable progress. Job terminals use the
    /// reserved path. PCM refusal poisons that job's terminal exactly once.
    pub fn send(&self, event: impl Into<BackendEvent>) {
        self.try_send(event.into());
    }

    pub fn try_send(&self, event: BackendEvent) -> bool {
        if !self.is_active() {
            return false;
        }
        let stream_guard = if matches!(event, BackendEvent::TtsChunk { .. }) {
            self.lease
                .as_ref()
                .map(|lease| lease.charge.lock().unwrap_or_else(|e| e.into_inner()))
        } else {
            None
        };
        if let Some(lease) = &self.lease {
            if matches!(event, BackendEvent::TtsChunk { .. }) && !event.same_job(&lease.fallback) {
                return false;
            }
            if event.is_terminal() {
                if !event.same_job(&lease.fallback) {
                    return false;
                }
                lease.publish(event);
                return true;
            }
            if matches!(event, BackendEvent::TtsChunk { .. })
                && (lease.failed.load(Ordering::Acquire)
                    || stream_guard.as_ref().is_some_and(|charge| charge.is_none()))
            {
                return false;
            }
        }
        if event.is_terminal() {
            // Explicit injection/fixture seam. Real providers reserve before
            // returning accepted submission, never rely on this convenience.
            let Some(job) = self.reserve(event.cancellation()) else {
                return false;
            };
            return job.try_send(event);
        }
        #[cfg(test)]
        if matches!(event, BackendEvent::TtsChunk { .. }) {
            if let Some((admitted, resume)) = self.shared.chunk_latch.lock().unwrap().take() {
                admitted.send(()).unwrap();
                resume.recv().unwrap();
            }
        }
        let valid = event.valid_nonterminal();
        let charge = valid
            .then(|| self.charge(event.retained_bytes(), false))
            .flatten();
        let Some(charge) = charge else {
            if matches!(event, BackendEvent::TtsChunk { .. }) {
                if let Some(lease) = &self.lease {
                    lease.failed.store(true, Ordering::Release);
                    drop(stream_guard);
                    lease.publish(lease.fallback.clone());
                }
            }
            return false;
        };
        self.sender
            .try_send(Queued {
                event,
                _charge: charge,
            })
            .is_ok()
    }

    pub fn usage(&self) -> MailboxUsage {
        *self.shared.usage.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// A refused submission owes no outcome. Accepted work must finish/drop.
    pub(crate) fn cancel_reservation(&self) {
        if let Some(lease) = &self.lease {
            lease
                .charge
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .take();
        }
    }

    /// Immediate admission/callback fence. No runtime join or filesystem work.
    pub fn retire(&self) {
        self.shared.active.store(false, Ordering::Release);
        self.shared.retirement.notify_waiters();
    }
}

#[derive(Debug, Clone)]
pub struct BackendReceiver {
    receiver: Receiver<Queued>,
    shared: Arc<Shared>,
}

impl BackendReceiver {
    pub fn generation(&self) -> RuntimeGeneration {
        self.shared.generation
    }
    pub fn len(&self) -> usize {
        self.receiver.len()
    }
    pub fn is_empty(&self) -> bool {
        self.receiver.is_empty()
    }

    pub fn try_recv_envelope(&self) -> Result<RuntimeEnvelope<BackendEvent>, TryRecvError> {
        self.try_recv().map(|event| self.generation().bind(event))
    }

    /// This receiver is permanently generation-bound. A host forwarding plain
    /// events must preserve `generation()`; it cannot rebind this endpoint.
    pub fn try_recv(&self) -> Result<BackendEvent, TryRecvError> {
        self.receiver.try_recv().map(|queued| queued.event)
    }

    pub fn recv_timeout(&self, timeout: Duration) -> Result<BackendEvent, RecvTimeoutError> {
        self.receiver
            .recv_timeout(timeout)
            .map(|queued| queued.event)
    }

    /// Snapshot length, finite even when workers continuously publish.
    pub fn try_iter(&self) -> impl Iterator<Item = BackendEvent> + '_ {
        (0..self.len().min(CALLBACK_DRAIN_CAPACITY)).map_while(|_| self.try_recv().ok())
    }

    pub fn retire(&self) {
        self.shared.active.store(false, Ordering::Release);
        self.shared.retirement.notify_waiters();
        for _ in 0..CALLBACK_CAPACITY {
            if self.receiver.try_recv().is_err() {
                break;
            }
        }
    }
}

pub fn backend_channel_for(generation: RuntimeGeneration) -> (BackendSender, BackendReceiver) {
    let (sender, receiver) = bounded(CALLBACK_CAPACITY);
    let shared = Arc::new(Shared {
        generation,
        active: AtomicBool::new(true),
        usage: Mutex::new(MailboxUsage::default()),
        retirement: tokio::sync::Notify::new(),
        #[cfg(test)]
        chunk_latch: Mutex::new(None),
    });
    (
        BackendSender {
            sender,
            shared: Arc::clone(&shared),
            lease: None,
        },
        BackendReceiver { receiver, shared },
    )
}

pub fn backend_channel() -> (BackendSender, BackendReceiver) {
    backend_channel_for(RuntimeGeneration::INITIAL)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cathedral_sim::{
        CognitionError, Completion, RequestId, SpeechError, SpeechEventId, StatusEvent,
    };

    fn completion(id: u64) -> BackendEvent {
        BackendEvent::LlmCompletion(Completion {
            request_id: RequestId(id),
            result: Ok("wait {}".into()),
            duration_seconds: 0.0,
        })
    }
    fn voice(id: &str) -> BackendEvent {
        BackendEvent::TtsDone {
            event_id: SpeechEventId(id.into()),
            result: Err(SpeechError::new("test")),
        }
    }
    fn chunk(id: &str, seq: u32) -> BackendEvent {
        BackendEvent::TtsChunk {
            event_id: SpeechEventId(id.into()),
            seq,
            sample_rate: 24_000,
            samples: Arc::from(vec![0i16; crate::MAX_PCM_CHUNK_BYTES / 2]),
        }
    }

    #[test]
    fn queued_terminals_keep_reserved_capacity_until_consumed() {
        let (sender, receiver) = backend_channel();
        let jobs: Vec<_> = (0..TERMINAL_CAPACITY)
            .map(|id| sender.reserve(completion(id as u64)).unwrap())
            .collect();
        assert!(sender.reserve(completion(100)).is_none());
        for id in 0..CALLBACK_CAPACITY - TERMINAL_CAPACITY {
            assert!(sender.try_send(StatusEvent::llm("busy", None, Some(id.to_string())).into()));
        }
        assert!(!sender.try_send(StatusEvent::llm("idle", None, None).into()));
        for (id, job) in jobs.iter().enumerate() {
            job.send(completion(id as u64));
        }
        drop(jobs);
        assert_eq!(sender.usage().records, CALLBACK_CAPACITY);
        assert_eq!(sender.usage().terminals, TERMINAL_CAPACITY);
        let metadata_bound = CALLBACK_CAPACITY * (std::mem::size_of::<Queued>() + 64)
            + TERMINAL_CAPACITY * (std::mem::size_of::<Lease>() + 32 + 4096)
            + std::mem::size_of::<Shared>()
            + 4096;
        assert!(metadata_bound < 384 * 1024);
        eprintln!(
            "M1c mailbox full-window usage={:?} queued_record_size={} lease_size={} conservative_channel_and_lease_metadata_bytes={metadata_bound}",
            sender.usage(),
            std::mem::size_of::<Queued>(),
            std::mem::size_of::<Lease>()
        );
        assert!(sender.reserve(completion(100)).is_none());
        for _ in 0..CALLBACK_CAPACITY - TERMINAL_CAPACITY {
            receiver.try_recv().unwrap();
        }
        assert!(sender.reserve(completion(100)).is_none());
        receiver.try_recv().unwrap();
        let next = sender.reserve(completion(100)).unwrap();
        next.cancel_reservation();
        for _ in 0..TERMINAL_CAPACITY - 1 {
            receiver.try_recv().unwrap();
        }
        assert_eq!(sender.usage(), MailboxUsage::default());
    }

    #[test]
    fn producer_loss_is_failure_even_when_reservation_was_success_shaped() {
        let (sender, receiver) = backend_channel();
        let job = sender.reserve(completion(7)).unwrap();
        drop(job);
        let BackendEvent::LlmCompletion(result) = receiver.try_recv().unwrap() else {
            panic!()
        };
        assert_eq!(result.request_id, RequestId(7));
        assert!(result.result.is_err());
        assert!(receiver.try_recv().is_err());
    }

    #[test]
    fn pcm_overflow_commits_one_failure_and_rejects_late_chunks_or_end() {
        let (sender, receiver) = backend_channel();
        let job = sender.reserve(voice("same-id")).unwrap();
        assert!(!job.try_send(chunk("different-id", 0)));
        for seq in 0..15 {
            assert!(job.try_send(chunk("same-id", seq)));
        }
        // Each PCM record also retains its event ID and fixed payload metadata.
        assert!(!job.try_send(chunk("same-id", 15)));
        assert!(!job.try_send(chunk("same-id", 16)));
        job.send(BackendEvent::TtsStreamEnd {
            event_id: SpeechEventId("same-id".into()),
            chunk_count: 16,
            first_chunk_ms: 0,
        });
        drop(job);
        let events: Vec<_> = receiver.try_iter().collect();
        assert_eq!(events.len(), 16);
        assert!(
            events[..15]
                .iter()
                .all(|event| matches!(event, BackendEvent::TtsChunk { .. }))
        );
        assert!(matches!(
            events.last(),
            Some(BackendEvent::TtsDone { result: Err(_), .. })
        ));
        assert_eq!(sender.usage(), MailboxUsage::default());
    }

    #[test]
    fn concurrent_chunk_and_terminal_never_publish_pcm_after_terminal() {
        let (sender, receiver) = backend_channel();
        let job = sender.reserve(voice("voice-1")).unwrap();
        let peer = job.clone();
        let (admitted, entered) = bounded(1);
        let (resume, release) = bounded(1);
        *sender.shared.chunk_latch.lock().unwrap() = Some((admitted, release));
        let task = std::thread::spawn(move || {
            peer.try_send(chunk("voice-1", 0));
        });
        entered.recv().unwrap();
        // Controlled at the old check/enqueue gap: admission must still
        // own the terminal mutex, independent of OS thread scheduling.
        assert!(job.lease.as_ref().unwrap().charge.try_lock().is_err());
        let (attempted, attempt) = bounded(1);
        let end = std::thread::spawn(move || {
            attempted.send(()).unwrap();
            job.send(BackendEvent::TtsStreamEnd {
                event_id: SpeechEventId("voice-1".into()),
                chunk_count: 1,
                first_chunk_ms: 0,
            });
        });
        attempt.recv().unwrap();
        assert!(receiver.try_recv().is_err());
        resume.send(()).unwrap();
        task.join().unwrap();
        end.join().unwrap();
        let events: Vec<_> = receiver.try_iter().collect();
        assert_eq!(events.len(), 2);
        assert!(matches!(
            events.first(),
            Some(BackendEvent::TtsChunk { .. })
        ));
        assert!(matches!(
            events.last(),
            Some(BackendEvent::TtsStreamEnd { .. })
        ));
    }

    #[test]
    fn retiring_and_dropping_nonempty_channels_releases_allocations() {
        let (sender, receiver) = backend_channel_for(RuntimeGeneration(11));
        let weak = Arc::downgrade(&sender.shared);
        let job = sender.reserve(voice("voice-1")).unwrap();
        job.send(chunk("voice-1", 0));
        job.send(voice("voice-1"));
        drop(job);
        drop(sender);
        drop(receiver);
        assert!(
            weak.upgrade().is_none(),
            "queued charges must not own the channel"
        );

        let (old, old_events) = backend_channel_for(RuntimeGeneration(12));
        let pending = old.reserve(completion(1)).unwrap();
        old_events.retire();
        pending.send(completion(1));
        drop(pending);
        assert!(old_events.try_recv().is_err());
        assert!(old.reserve(completion(2)).is_none());
        let (current, current_events) = backend_channel_for(RuntimeGeneration(13));
        current.send(completion(1));
        assert_eq!(
            current_events.try_recv_envelope().unwrap().generation,
            RuntimeGeneration(13)
        );
    }

    #[test]
    fn spare_capacity_and_largest_terminal_payloads_obey_allocated_byte_budget() {
        let (sender, receiver) = backend_channel();
        let job = sender.reserve(completion(8)).unwrap();
        let mut detail = String::with_capacity(2 * 1024 * 1024);
        detail.push_str("small message");
        job.send(BackendEvent::LlmCompletion(Completion {
            request_id: RequestId(8),
            result: Err(CognitionError::detailed("test", detail)),
            duration_seconds: 0.0,
        }));
        let BackendEvent::LlmCompletion(result) = receiver.try_recv().unwrap() else {
            panic!()
        };
        assert!(result.result.unwrap_err().detail().contains("cancelled"));
        let a = sender.reserve(voice("a")).unwrap();
        let b = sender.reserve(voice("b")).unwrap();
        let c = sender.reserve(voice("c")).unwrap();
        assert!(sender.reserve(voice("d")).is_none());
        assert!(sender.usage().terminal_bytes <= TERMINAL_BYTE_CAPACITY);
        drop((a, b, c));
        assert_eq!(sender.usage().terminals, 3);
        receiver.try_iter().for_each(drop);
        assert_eq!(sender.usage(), MailboxUsage::default());
    }

    #[test]
    fn drain_cohort_is_finite_while_a_producer_continues() {
        let (sender, receiver) = backend_channel();
        sender.send(StatusEvent::llm("idle", None, None));
        let mut cohort = receiver.try_iter();
        assert!(cohort.next().is_some());
        sender.send(StatusEvent::llm("busy", None, None));
        assert!(cohort.next().is_none());
        assert_eq!(receiver.len(), 1);
    }
}
