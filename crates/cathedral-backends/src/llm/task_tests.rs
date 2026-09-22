use super::*;
use cathedral_sim::checkpoint::{CheckpointBudget, Cohort, RetirementLease, RetirementRelease};

const FIXTURE_BYTES: usize = 4096;

#[derive(Clone, Copy)]
enum Exit {
    NeverPolled,
    Pending,
    Finished,
    Retired,
    Panic,
}

/// This future deliberately keeps its payload even after returning Ready, as
/// real completed async futures are allowed to keep their captures.
struct Work {
    lane: Arc<AtomicBool>,
    released: RetirementRelease,
    disposed: Arc<AtomicBool>,
    payload: Vec<u8>,
    exit: Exit,
}

impl Future for Work {
    type Output = Option<Result<String, LlmError>>;

    fn poll(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Self::Output> {
        match self.exit {
            Exit::NeverPolled => panic!("must not be polled"),
            Exit::Pending => Poll::Pending,
            Exit::Finished => Poll::Ready(Some(Ok("finished".into()))),
            Exit::Retired => Poll::Ready(None),
            Exit::Panic => panic!("synthetic request panic"),
        }
    }
}

impl Drop for Work {
    fn drop(&mut self) {
        assert!(
            self.lane.load(Ordering::SeqCst),
            "lane released before payload"
        );
        assert!(
            !self.released.released(),
            "generation lease released before payload"
        );
        assert_eq!(self.payload.len(), FIXTURE_BYTES);
        self.disposed.store(true, Ordering::SeqCst);
    }
}

fn witness(exit: Exit) {
    // First retain a consumer to verify terminal conservation. Then remove all
    // other endpoints so only the task guard can keep the real retirement pin.
    for retain_receiver in [true, false] {
        let budget = CheckpointBudget::default();
        let lease = RetirementLease::new(
            budget
                .reserve(Cohort::RetiringGeneration, FIXTURE_BYTES)
                .unwrap(),
        )
        .unwrap();
        let released = lease.release_observer();
        let (sender, receiver) = crate::backend_channel();
        receiver.pin_retirement(&budget, &lease).unwrap();
        let fallback = crate::BackendEvent::LlmCompletion(Completion {
            request_id: RequestId(77),
            result: Err(CognitionError::new("reserved fallback")),
            duration_seconds: 0.0,
        });
        let delivery = sender.reserve(fallback).unwrap();
        let lane = Arc::new(AtomicBool::new(true));
        let disposed = Arc::new(AtomicBool::new(false));
        let work = Work {
            lane: lane.clone(),
            released: released.clone(),
            disposed: disposed.clone(),
            payload: vec![1; FIXTURE_BYTES],
            exit,
        };
        let mut task =
            CompletionTask::new(work, LaneGuard::new(lane.clone(), delivery, RequestId(77)));
        drop((sender, lease));
        let receiver = retain_receiver.then_some(receiver);
        assert_eq!(budget.retained_bytes(), FIXTURE_BYTES);
        let mut context = Context::from_waker(std::task::Waker::noop());
        match exit {
            Exit::NeverPolled => {}
            Exit::Panic => {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let _ = Pin::new(&mut task).poll(&mut context);
                }));
                assert!(result.is_err());
            }
            Exit::Pending => assert!(Pin::new(&mut task).poll(&mut context).is_pending()),
            Exit::Finished | Exit::Retired => {
                assert!(Pin::new(&mut task).poll(&mut context).is_ready())
            }
        }
        drop(task);
        assert!(disposed.load(Ordering::SeqCst));
        assert!(!lane.load(Ordering::SeqCst));
        if let Some(receiver) = receiver {
            let crate::BackendEvent::LlmCompletion(completion) = receiver.try_recv().unwrap()
            else {
                panic!("wrong terminal kind");
            };
            assert_eq!(completion.request_id, RequestId(77));
            assert_eq!(completion.result.is_ok(), matches!(exit, Exit::Finished));
            assert!(receiver.try_recv().is_err(), "duplicate terminal");
            assert!(!released.released(), "consumer still retains generation");
            drop(receiver);
        }
        assert!(released.released());
        assert_eq!(budget.retained_bytes(), 0);
    }
}

#[test]
fn never_polled_task_disposes_payload_before_terminal_and_retirement_pin() {
    witness(Exit::NeverPolled);
}

#[test]
fn pending_task_cancellation_disposes_payload_before_terminal_and_retirement_pin() {
    witness(Exit::Pending);
}

#[test]
fn finished_task_disposes_retained_captures_before_terminal_and_retirement_pin() {
    witness(Exit::Finished);
}

#[test]
fn retired_task_disposes_retained_captures_before_terminal_and_retirement_pin() {
    witness(Exit::Retired);
}

#[test]
fn panicking_task_disposes_payload_before_terminal_and_retirement_pin() {
    witness(Exit::Panic);
}
