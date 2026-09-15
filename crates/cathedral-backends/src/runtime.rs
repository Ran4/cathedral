//! The one tokio runtime the backends own (D23).
//!
//! cathedral-sim is single-threaded, clock-free and IO-free; every network
//! call, subprocess and timer in the port lives on this runtime instead. The
//! host holds an [`Arc<BackendRuntime>`] inside its `BackendsHandle` and drops
//! it off-frame on shutdown, joining the workers before releasing admission.

use std::{future::Future, io, sync::Arc};

use cathedral_sim::checkpoint::{CheckpointBudget, Reservation};

use crate::{BackendSender, dns::DnsPool};

pub const ASYNC_WORKERS: usize = 2;
pub const BLOCKING_WORKERS: usize = 2;
pub const NATIVE_STACK_BYTES: usize = 2 * 1024 * 1024;
/// Four explicit stacks (8 MiB), bounded resolver native scratch (4 MiB),
/// and a trusted 4 MiB runtime/allocator/TLS allowance, not a process heap census.
pub const RUNTIME_ALLOWANCE_BYTES: usize = 16 * 1024 * 1024;

use tokio::runtime::{Builder, Handle, Runtime};
use tokio::task::JoinHandle;

/// A multi-threaded tokio runtime with a small, named worker pool.
///
/// Small on purpose: the concurrent work is a handful of HTTP requests, a
/// websocket and a few subprocess pipes — never CPU-bound.
pub struct BackendRuntime {
    /// `Option` only so [`Drop`] can take it and join its native workers;
    /// it is `Some` for the whole life of the value.
    runtime: Option<Runtime>,
    // Survives Runtime destruction AND retained DNS answers.
    dns: Arc<DnsPool>,
}

/// A production executor retains shared controls but cannot destroy/join the
/// Runtime from one of its own workers. Field order releases the raw Handle first.
#[derive(Clone)]
pub(crate) struct BackendExecutor {
    handle: Handle,
    _dns: Arc<DnsPool>,
}
impl BackendExecutor {
    pub(crate) fn block_on<F: Future>(&self, future: F) -> F::Output {
        self.handle.block_on(future)
    }
}

impl BackendRuntime {
    /// Start the runtime. Fails only if the OS refuses the threads.
    pub fn new() -> io::Result<Arc<Self>> {
        Self::start(None)
    }

    /// Startup admission from the caller's existing shared budget. The final
    /// owner must be disposed off-frame; no generation is assigned to this pool.
    pub fn start_admitted(
        budget: &CheckpointBudget,
    ) -> cathedral_sim::checkpoint::Result<Arc<Self>> {
        let lease = budget.reserve_running_overhead(RUNTIME_ALLOWANCE_BYTES)?;
        Self::start(Some(lease)).map_err(|_| cathedral_sim::checkpoint::CheckpointError {
            owner: "runtime",
            reason: "backend runtime startup failed".into(),
        })
    }

    fn start(lease: Option<Reservation>) -> io::Result<Arc<Self>> {
        let dns = Arc::new(DnsPool::new(lease));
        let runtime = Builder::new_multi_thread()
            .worker_threads(ASYNC_WORKERS)
            .max_blocking_threads(BLOCKING_WORKERS)
            .thread_stack_size(NATIVE_STACK_BYTES)
            .thread_name("cathedral-backends")
            .enable_all()
            .build()?;
        Ok(Arc::new(Self {
            runtime: Some(runtime),
            dns,
        }))
    }

    pub(crate) fn resolver(&self, events: BackendSender) -> crate::dns::NativeResolver {
        crate::dns::NativeResolver::new(Arc::clone(&self.dns), Some(events))
    }

    pub(crate) fn executor(&self) -> BackendExecutor {
        BackendExecutor {
            handle: self.handle().clone(),
            _dns: Arc::clone(&self.dns),
        }
    }

    pub fn handle(&self) -> &Handle {
        self.runtime
            .as_ref()
            .expect("runtime is taken only in Drop")
            .handle()
    }

    /// Spawn a backend task. The returned handle is usually dropped: results
    /// travel back over the [`BackendEvent`](crate::events::BackendEvent)
    /// channel, not by joining.
    pub fn spawn<F>(&self, future: F) -> JoinHandle<F::Output>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        self.handle().spawn(future)
    }

    /// Spawn blocking work (subprocess pipes, file IO) off the async workers.
    #[cfg(test)]
    pub(crate) fn spawn_blocking<F, R>(&self, task: F) -> JoinHandle<R>
    where
        F: FnOnce() -> R + Send + 'static,
        R: Send + 'static,
    {
        self.handle().spawn_blocking(task)
    }

    /// Run a future to completion from a synchronous caller (the one-shot CLI
    /// and tests). Never call this from inside a runtime thread.
    pub fn block_on<F: Future>(&self, future: F) -> F::Output {
        self.runtime
            .as_ref()
            .expect("runtime is taken only in Drop")
            .block_on(future)
    }
}

impl Drop for BackendRuntime {
    fn drop(&mut self) {
        if let Some(runtime) = self.runtime.take() {
            // Final destruction is off-frame, through retirement/failed-service
            // disposal. Tokio aborts async tasks and joins every native worker,
            // including running DNS that cannot be aborted. Its blocking pool
            // joins native JoinHandles after shutdown notification, so the
            // shared allowance outlives actual OS stack teardown. Backend task
            // closures hold Handle/client/endpoints, never this runtime owner.
            drop(runtime);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn spawned_tasks_run_and_block_on_returns_their_value() {
        let runtime = BackendRuntime::new().expect("runtime starts");
        let task = runtime.spawn(async { 40 + 2 });
        assert_eq!(runtime.block_on(task).expect("task completes"), 42);
    }

    #[test]
    fn blocking_work_runs_off_the_async_workers() {
        let runtime = BackendRuntime::new().expect("runtime starts");
        let task = runtime.spawn_blocking(|| "done");
        assert_eq!(runtime.block_on(task).expect("task completes"), "done");
    }

    #[test]
    fn dropping_the_runtime_does_not_wait_for_a_hung_task() {
        let runtime = BackendRuntime::new().expect("runtime starts");
        runtime.spawn(async {
            tokio::time::sleep(Duration::from_secs(3_600)).await;
        });
        let started = std::time::Instant::now();
        drop(runtime);
        assert!(started.elapsed() < Duration::from_secs(2));
    }
}
