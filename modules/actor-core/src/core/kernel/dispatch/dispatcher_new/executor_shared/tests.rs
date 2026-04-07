use alloc::{boxed::Box, sync::Arc};
use core::sync::atomic::{AtomicUsize, Ordering};

use super::ExecutorShared;
use crate::core::kernel::dispatch::dispatcher_new::{ExecuteError, Executor};

struct CountingExecutor {
  count:    Arc<AtomicUsize>,
  blocking: bool,
}

impl Executor for CountingExecutor {
  fn execute(&mut self, task: Box<dyn FnOnce() + Send + 'static>) -> Result<(), ExecuteError> {
    self.count.fetch_add(1, Ordering::SeqCst);
    task();
    Ok(())
  }

  fn supports_blocking(&self) -> bool {
    self.blocking
  }

  fn shutdown(&mut self) {
    self.count.store(0, Ordering::SeqCst);
  }
}

struct RejectingExecutor;

impl Executor for RejectingExecutor {
  fn execute(&mut self, _task: Box<dyn FnOnce() + Send + 'static>) -> Result<(), ExecuteError> {
    Err(ExecuteError::Rejected)
  }

  fn shutdown(&mut self) {}
}

#[test]
fn execute_delegates_to_inner() {
  let count = Arc::new(AtomicUsize::new(0));
  let shared = ExecutorShared::new(CountingExecutor { count: Arc::clone(&count), blocking: true });
  let observed = Arc::new(AtomicUsize::new(0));
  let observed_clone = Arc::clone(&observed);
  shared
    .execute(Box::new(move || {
      observed_clone.store(1, Ordering::SeqCst);
    }))
    .expect("execute should succeed");
  assert_eq!(count.load(Ordering::SeqCst), 1);
  assert_eq!(observed.load(Ordering::SeqCst), 1);
}

#[test]
fn execute_propagates_errors() {
  let shared = ExecutorShared::new(RejectingExecutor);
  let result = shared.execute(Box::new(|| {}));
  assert!(matches!(result, Err(ExecuteError::Rejected)));
}

#[test]
fn supports_blocking_query() {
  let count = Arc::new(AtomicUsize::new(0));
  let shared = ExecutorShared::new(CountingExecutor { count, blocking: false });
  assert!(!shared.supports_blocking());
}

#[test]
fn shutdown_invokes_inner_shutdown() {
  let count = Arc::new(AtomicUsize::new(0));
  let shared = ExecutorShared::new(CountingExecutor { count: Arc::clone(&count), blocking: true });
  shared.execute(Box::new(|| {})).expect("execute should succeed");
  assert_eq!(count.load(Ordering::SeqCst), 1);
  shared.shutdown();
  assert_eq!(count.load(Ordering::SeqCst), 0);
}

#[test]
fn clone_shares_inner_state() {
  let count = Arc::new(AtomicUsize::new(0));
  let shared = ExecutorShared::new(CountingExecutor { count: Arc::clone(&count), blocking: true });
  let cloned = shared.clone();
  shared.execute(Box::new(|| {})).expect("execute should succeed");
  cloned.execute(Box::new(|| {})).expect("execute should succeed");
  assert_eq!(count.load(Ordering::SeqCst), 2);
}
