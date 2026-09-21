//! Independent coordinator-owned diagnostic boundary tests.

use super::bounded::{BoundedSink, Refusal, RECORD_BYTES};
use std::io::{self, Write};
use std::time::{Duration, Instant};

struct Sink;
impl Write for Sink {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> { Ok(bytes.len()) }
    fn flush(&mut self) -> io::Result<()> { Ok(()) }
}

#[test]
fn coordinator_sink_refuses_oversized_and_reports_usage() {
    let sink = BoundedSink::start(Sink, None, 1, 1, false).unwrap();
    let sender = sink.sender();
    let oversized = "x".repeat(RECORD_BYTES);
    let err = sender.line_owned(oversized).unwrap_err();
    assert_eq!(err.0, Refusal::Oversized);
    assert_eq!(sender.usage().refused_oversized, 1);
    sender.close();
}

#[test]
fn coordinator_sink_fence_completes_after_admitted_line() {
    let sink = BoundedSink::start(Sink, None, 1, 1, false).unwrap();
    let sender = sink.sender();
    let ticket = sender.line_owned("diagnostic".into()).unwrap();
    sender.fence(Some(ticket), Instant::now() + Duration::from_secs(1)).unwrap();
    assert_eq!(sender.usage().completed, 1);
    sender.close();
}
