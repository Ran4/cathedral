//! Fixed storage covers formatting, queued and active diagnostics together.
use std::{
    io::{self, Write},
    sync::{Arc, Condvar, Mutex},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use cathedral_sim::checkpoint::{CheckpointBudget, Reservation};
use crossbeam_channel::{Receiver, Sender, bounded};

pub(crate) const RECORD_BYTES: usize = 32 * 1024;
const HEADER_BYTES: usize = 128;
pub(crate) const DIAGNOSTIC_SLOTS: usize = 64;
pub(crate) const EVIDENCE_SLOTS: usize = 4;
pub(crate) const STACK_BYTES: usize = 2 * 1024 * 1024;
/// Per sink: 68 * 64 KiB buffers, 2 MiB stack, remaining fixed/channel/TLS scope.
pub(crate) const ALLOWANCE_BYTES: usize = 8 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Refusal {
    Full,
    Oversized,
    Closed,
    Io,
    Timeout,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Ticket(pub u64);
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Usage {
    pub formatting: usize,
    pub queued: usize,
    pub active: usize,
    pub accepted: u64,
    pub completed: u64,
    pub refused_full: u64,
    pub refused_oversized: u64,
    pub refused_closed: u64,
    pub io_failed: bool,
    pub closed: bool,
}
struct Progress {
    usage: Usage,
    last_epoch_ms: u64,
    reported: (u64, u64, u64),
}
struct Core {
    progress: Mutex<Progress>,
    changed: Condvar,
    // Kept through worker join, formatting cancellation and endpoint disposal.
    _lease: Option<Reservation>,
}

struct Buffer {
    encoded: Box<[u8]>,
    scratch: Box<[u8]>,
    len: usize,
}
impl Buffer {
    fn new() -> Self {
        Self {
            encoded: vec![0; RECORD_BYTES].into_boxed_slice(),
            scratch: vec![0; RECORD_BYTES].into_boxed_slice(),
            len: 0,
        }
    }
}

/// No joining owner is held by a draft, worker or producer endpoint.
pub(crate) struct Draft {
    buffer: Option<Buffer>,
    recycle: Sender<Buffer>,
    core: Arc<Core>,
    formatting: bool,
}
impl Draft {
    pub(crate) fn scratch(&mut self) -> &mut [u8] {
        &mut self.buffer.as_mut().unwrap().scratch
    }
    pub(crate) fn parts(&mut self) -> (&[u8], &mut [u8]) {
        let b = self.buffer.as_mut().unwrap();
        (&b.scratch, &mut b.encoded[HEADER_BYTES..RECORD_BYTES - 1])
    }
    pub(crate) fn set_body_len(&mut self, len: usize) {
        self.buffer.as_mut().unwrap().len = len;
    }
    pub(crate) fn console_body(&mut self) {
        let b = self.buffer.as_mut().unwrap();
        b.encoded.copy_within(HEADER_BYTES..HEADER_BYTES + b.len, 0);
        b.encoded[b.len] = b'\n';
        b.len += 1;
    }
    pub(crate) fn text(&mut self, text: &str) -> Result<(), Refusal> {
        if text.len() >= RECORD_BYTES {
            return Err(Refusal::Oversized);
        }
        let b = self.buffer.as_mut().unwrap();
        b.encoded[..text.len()].copy_from_slice(text.as_bytes());
        b.encoded[text.len()] = b'\n';
        b.len = text.len() + 1;
        Ok(())
    }
    fn finish_json(&mut self, epoch_ms: u64) -> Result<(), Refusal> {
        let stamp = super::timestamp_from_unix_seconds(epoch_ms / 1000);
        let b = self.buffer.as_mut().unwrap();
        let mut header = SliceWriter::new(&mut b.encoded[..HEADER_BYTES]);
        write!(
            header,
            "{{\"ts\":\"{:04}-{:02}-{:02} {:02}:{:02}:{:02}.{:03}\",\"ts_ms\":{}",
            stamp.year,
            stamp.month,
            stamp.day,
            stamp.hour,
            stamp.minute,
            stamp.second,
            epoch_ms % 1000,
            epoch_ms
        )
        .map_err(|_| Refusal::Oversized)?;
        let prefix = header.len;
        if b.len < 2 || b.encoded[HEADER_BYTES] != b'{' {
            return Err(Refusal::Oversized);
        }
        // Replace the body's opening brace with a comma after the timestamp.
        b.encoded[prefix] = b',';
        b.encoded
            .copy_within(HEADER_BYTES + 1..HEADER_BYTES + b.len, prefix + 1);
        b.len += prefix;
        b.encoded[b.len] = b'\n';
        b.len += 1;
        Ok(())
    }
}
impl Drop for Draft {
    fn drop(&mut self) {
        if self.formatting {
            self.core.progress.lock().unwrap().usage.formatting -= 1;
        }
        if let Some(mut b) = self.buffer.take() {
            b.len = 0;
            // The pool owns every buffer; a closed last receiver disposes it
            // before this draft's final core/allowance owner is released.
            let _ = self.recycle.try_send(b);
        }
        self.core.changed.notify_all();
    }
}
struct Job {
    draft: Draft,
    sequence: u64,
}

#[derive(Clone)]
pub(crate) struct SinkSender {
    jobs: Sender<Job>,
    diagnostic: Receiver<Buffer>,
    evidence: Receiver<Buffer>,
    diagnostic_return: Sender<Buffer>,
    evidence_return: Sender<Buffer>,
    core: Arc<Core>,
}
pub(crate) struct BoundedSink {
    // Payload/channel storage precedes the owner retaining native termination.
    sender: SinkSender,
    worker: Option<JoinHandle<()>>,
    _core: Arc<Core>,
}
impl BoundedSink {
    pub(crate) fn start_admitted(
        writer: impl Write + Send + 'static,
        budget: &CheckpointBudget,
    ) -> cathedral_sim::checkpoint::Result<Self> {
        let lease = budget.reserve_running_overhead(ALLOWANCE_BYTES)?;
        Self::start(writer, Some(lease), DIAGNOSTIC_SLOTS, EVIDENCE_SLOTS, false).map_err(|_| {
            cathedral_sim::checkpoint::CheckpointError {
                owner: "diagnostics",
                reason: "worker startup failed".into(),
            }
        })
    }
    pub(crate) fn start_default(writer: impl Write + Send + 'static) -> io::Result<Self> {
        Self::start(writer, None, DIAGNOSTIC_SLOTS, EVIDENCE_SLOTS, false)
    }
    pub(crate) fn start(
        mut writer: impl Write + Send + 'static,
        lease: Option<Reservation>,
        diagnostic_slots: usize,
        evidence_slots: usize,
        fail_spawn: bool,
    ) -> io::Result<Self> {
        assert!(diagnostic_slots > 0 && diagnostic_slots <= DIAGNOSTIC_SLOTS);
        assert!(evidence_slots > 0 && evidence_slots <= EVIDENCE_SLOTS);
        let core = Arc::new(Core {
            progress: Mutex::new(Progress {
                usage: Usage::default(),
                last_epoch_ms: 0,
                reported: (0, 0, 0),
            }),
            changed: Condvar::new(),
            _lease: lease,
        });
        let (diagnostic_return, diagnostic) = bounded(diagnostic_slots);
        let (evidence_return, evidence) = bounded(evidence_slots);
        for _ in 0..diagnostic_slots {
            diagnostic_return.send(Buffer::new()).unwrap();
        }
        for _ in 0..evidence_slots {
            evidence_return.send(Buffer::new()).unwrap();
        }
        let (jobs, receiver) = bounded::<Job>(diagnostic_slots + evidence_slots);
        let sender = SinkSender {
            jobs,
            diagnostic,
            evidence,
            diagnostic_return,
            evidence_return,
            core: core.clone(),
        };
        if fail_spawn {
            drop(writer);
            return Err(io::Error::other("controlled worker startup failure"));
        }
        let worker_core = core.clone();
        let worker = thread::Builder::new()
            .name("cathedral-diagnostic".into())
            .stack_size(STACK_BYTES)
            .spawn(move || {
                let _completion = WorkerCompletion {
                    receiver: &receiver,
                    core: &worker_core,
                };
                loop {
                    let job = match receiver.recv_timeout(Duration::from_millis(10)) {
                        Ok(job) => job,
                        Err(crossbeam_channel::RecvTimeoutError::Timeout) => {
                            if worker_core.progress.lock().unwrap().usage.closed {
                                break;
                            }
                            continue;
                        }
                        Err(crossbeam_channel::RecvTimeoutError::Disconnected) => break,
                    };
                    {
                        let mut p = worker_core.progress.lock().unwrap();
                        p.usage.queued -= 1;
                        p.usage.active = 1;
                    }
                    let sequence = job.sequence;
                    let b = job.draft.buffer.as_ref().unwrap();
                    let written = writer
                        .write_all(&b.encoded[..b.len])
                        .and_then(|_| writer.flush());
                    drop(job);
                    let mut p = worker_core.progress.lock().unwrap();
                    p.usage.active = 0;
                    if written.is_err() {
                        p.usage.io_failed = true;
                        p.usage.closed = true;
                        worker_core.changed.notify_all();
                        break;
                    }
                    p.usage.completed = sequence;
                    worker_core.changed.notify_all();
                }
            })?;
        Ok(Self {
            sender,
            worker: Some(worker),
            _core: core,
        })
    }
    pub(crate) fn sender(&self) -> SinkSender {
        self.sender.clone()
    }
}
impl Drop for BoundedSink {
    fn drop(&mut self) {
        self.sender.close();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
        // core and its admission outlive the join, including native TLS teardown.
    }
}
struct WorkerCompletion<'a> {
    receiver: &'a Receiver<Job>,
    core: &'a Core,
}
impl Drop for WorkerCompletion<'_> {
    fn drop(&mut self) {
        {
            let mut p = self.core.progress.lock().unwrap();
            if p.usage.active != 0 {
                p.usage.io_failed = true;
            }
            p.usage.active = 0;
            p.usage.closed = true;
        }
        while let Ok(job) = self.receiver.try_recv() {
            self.core.progress.lock().unwrap().usage.queued -= 1;
            drop(job);
        }
        self.core.changed.notify_all();
    }
}
impl SinkSender {
    pub(crate) fn unreported_losses(&self) -> Option<(u64, u64, u64)> {
        let p = self.core.progress.lock().unwrap();
        let losses = (
            p.usage.refused_full,
            p.usage.refused_oversized,
            p.usage.refused_closed,
        );
        (losses != p.reported).then_some(losses)
    }
    pub(crate) fn mark_reported(&self, losses: (u64, u64, u64)) {
        let mut p = self.core.progress.lock().unwrap();
        p.reported = (
            p.reported.0.max(losses.0),
            p.reported.1.max(losses.1),
            p.reported.2.max(losses.2),
        );
    }
    pub(crate) fn usage(&self) -> Usage {
        self.core.progress.lock().unwrap().usage
    }
    pub(crate) fn close(&self) {
        self.core.progress.lock().unwrap().usage.closed = true;
        self.core.changed.notify_all();
    }
    pub(crate) fn refuse(&self, reason: Refusal) {
        let mut p = self.core.progress.lock().unwrap();
        let counter = match reason {
            Refusal::Full | Refusal::Timeout => &mut p.usage.refused_full,
            Refusal::Oversized => &mut p.usage.refused_oversized,
            _ => &mut p.usage.refused_closed,
        };
        *counter = counter.saturating_add(1);
    }
    pub(crate) fn begin(
        &self,
        evidence: bool,
        deadline: Option<Instant>,
    ) -> Result<Draft, Refusal> {
        let pool = if evidence {
            &self.evidence
        } else {
            &self.diagnostic
        };
        loop {
            let mut p = self.core.progress.lock().unwrap();
            if p.usage.closed {
                drop(p);
                self.refuse(Refusal::Closed);
                return Err(Refusal::Closed);
            }
            if let Ok(buffer) = pool.try_recv() {
                p.usage.formatting += 1;
                return Ok(Draft {
                    buffer: Some(buffer),
                    recycle: if evidence {
                        self.evidence_return.clone()
                    } else {
                        self.diagnostic_return.clone()
                    },
                    core: self.core.clone(),
                    formatting: true,
                });
            }
            let Some(deadline) = deadline else {
                drop(p);
                self.refuse(Refusal::Full);
                return Err(Refusal::Full);
            };
            let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
                drop(p);
                self.refuse(Refusal::Timeout);
                return Err(Refusal::Timeout);
            };
            drop(self.core.changed.wait_timeout(p, remaining).unwrap());
        }
    }
    pub(crate) fn commit(&self, mut draft: Draft, json: bool) -> Result<Ticket, Refusal> {
        assert!(Arc::ptr_eq(&self.core, &draft.core));
        let mut p = self.core.progress.lock().unwrap();
        if p.usage.closed {
            drop(p);
            self.refuse(Refusal::Closed);
            return Err(Refusal::Closed);
        }
        let Some(sequence) = p.usage.accepted.checked_add(1) else {
            p.usage.closed = true;
            drop(p);
            self.refuse(Refusal::Closed);
            return Err(Refusal::Closed);
        };
        if json {
            let epoch = super::now_epoch_milliseconds().max(p.last_epoch_ms);
            if let Err(reason) = draft.finish_json(epoch) {
                drop(p);
                self.refuse(reason);
                return Err(reason);
            }
            p.last_epoch_ms = epoch;
        }
        draft.formatting = false;
        p.usage.formatting -= 1;
        p.usage.queued += 1;
        // Slots include formatting and active owners, so the channel cannot fill.
        match self.jobs.try_send(Job { draft, sequence }) {
            Ok(()) => {
                p.usage.accepted = sequence;
                Ok(Ticket(sequence))
            }
            Err(error) => {
                p.usage.queued -= 1;
                p.usage.closed = true;
                drop(p);
                drop(error);
                self.refuse(Refusal::Closed);
                Err(Refusal::Closed)
            }
        }
    }
    pub(crate) fn line_owned(&self, line: String) -> Result<Ticket, (Refusal, String)> {
        if line.capacity() >= RECORD_BYTES {
            self.refuse(Refusal::Oversized);
            return Err((Refusal::Oversized, line));
        }
        let mut draft = match self.begin(false, None) {
            Ok(draft) => draft,
            Err(reason) => return Err((reason, line)),
        };
        // No retained input copy: accepted bytes fit the already admitted slot.
        if let Err(e) = draft.text(&line) {
            self.refuse(e);
            return Err((e, line));
        }
        self.commit(draft, false).map_err(|e| (e, line))
    }
    pub(crate) fn fence(&self, ticket: Option<Ticket>, deadline: Instant) -> Result<(), Refusal> {
        let mut p = self.core.progress.lock().unwrap();
        let target = ticket.map_or(p.usage.accepted, |t| t.0);
        if target > p.usage.accepted {
            return Err(Refusal::Closed);
        }
        loop {
            if p.usage.completed >= target {
                return Ok(());
            }
            if p.usage.io_failed || p.usage.closed && p.usage.active == 0 && p.usage.queued == 0 {
                return Err(Refusal::Io);
            }
            let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
                return Err(Refusal::Timeout);
            };
            p = self.core.changed.wait_timeout(p, remaining).unwrap().0;
        }
    }
}

pub(crate) struct SliceWriter<'a> {
    bytes: &'a mut [u8],
    pub len: usize,
}
impl<'a> SliceWriter<'a> {
    pub(crate) fn new(bytes: &'a mut [u8]) -> Self {
        Self { bytes, len: 0 }
    }
}
impl Write for SliceWriter<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > self.bytes.len() - self.len {
            return Err(io::Error::other("diagnostic record exceeds fixed storage"));
        }
        self.bytes[self.len..self.len + bytes.len()].copy_from_slice(bytes);
        self.len += bytes.len();
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
