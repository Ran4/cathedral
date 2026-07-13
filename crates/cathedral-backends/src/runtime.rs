//! The one tokio runtime the backends own (D23).
//!
//! cathedral-sim is single-threaded, clock-free and IO-free; every network
//! call, subprocess and timer in the port lives on this runtime instead. The
//! host holds an [`Arc<BackendRuntime>`] inside its `BackendsHandle` and drops
//! it on shutdown, which stops the workers.

use std::{future::Future, io, sync::Arc, time::Duration};

use tokio::runtime::{Builder, Handle, Runtime};
use tokio::task::JoinHandle;

/// How long a drop waits for in-flight tasks before abandoning them. A provider
/// call must never hold the game's exit open.
const SHUTDOWN_GRACE: Duration = Duration::from_millis(500);

/// A multi-threaded tokio runtime with a small, named worker pool.
///
/// Small on purpose: the concurrent work is a handful of HTTP requests, a
/// websocket and a few subprocess pipes — never CPU-bound.
pub struct BackendRuntime {
    /// `Option` only so [`Drop`] can take it and shut it down with a timeout;
    /// it is `Some` for the whole life of the value.
    runtime: Option<Runtime>,
}

impl BackendRuntime {
    /// Start the runtime. Fails only if the OS refuses the threads.
    pub fn new() -> io::Result<Arc<Self>> {
        let runtime = Builder::new_multi_thread()
            .worker_threads(2)
            .thread_name("cathedral-backends")
            .enable_all()
            .build()?;
        Ok(Arc::new(Self {
            runtime: Some(runtime),
        }))
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
    pub fn spawn_blocking<F, R>(&self, task: F) -> JoinHandle<R>
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
            // Abandon anything still running rather than blocking the caller
            // (a 45 s provider timeout must not delay quitting the game).
            runtime.shutdown_timeout(SHUTDOWN_GRACE);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
