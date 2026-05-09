extern crate std;

use alloc::{boxed::Box, sync::Arc};
use core::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Barrier;

use fraktor_actor_core_rs::dispatch::dispatcher::{ExecuteError, Executor};

use super::PinnedExecutor;

#[test]
fn execute_runs_tasks_serially_on_worker_thread() {
  let mut executor = PinnedExecutor::with_name("pinned-test");
  let count = Arc::new(AtomicUsize::new(0));
  let barrier = Arc::new(Barrier::new(2));
  let count_clone = Arc::clone(&count);
  let barrier_clone = Arc::clone(&barrier);
  executor
    .execute(
      Box::new(move || {
        count_clone.fetch_add(1, Ordering::SeqCst);
        barrier_clone.wait();
      }),
      0,
    )
    .expect("first submission");
  barrier.wait();
  assert_eq!(count.load(Ordering::SeqCst), 1);
  executor.shutdown();
}

#[test]
fn execute_after_shutdown_returns_error() {
  let mut executor = PinnedExecutor::with_name("pinned-test-shutdown");
  executor.shutdown();
  let result = executor.execute(Box::new(|| {}), 0);
  assert!(matches!(result, Err(ExecuteError::Shutdown)));
}
