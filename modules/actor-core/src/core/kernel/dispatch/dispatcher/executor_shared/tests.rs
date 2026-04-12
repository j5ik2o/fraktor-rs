use alloc::{boxed::Box, sync::Arc};
use core::sync::atomic::{AtomicUsize, Ordering};

use crate::core::kernel::{
  dispatch::dispatcher::{ExecuteError, Executor, ExecutorSharedFactory},
  system::shared_factory::BuiltinSpinSharedFactory,
};

struct CountingExecutor {
  count: Arc<AtomicUsize>,
}

impl Executor for CountingExecutor {
  fn execute(&mut self, task: Box<dyn FnOnce() + Send + 'static>) -> Result<(), ExecuteError> {
    self.count.fetch_add(1, Ordering::SeqCst);
    task();
    Ok(())
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
  let shared = ExecutorSharedFactory::create(
    &BuiltinSpinSharedFactory::new(),
    Box::new(CountingExecutor { count: Arc::clone(&count) }),
  );
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
  let shared = ExecutorSharedFactory::create(&BuiltinSpinSharedFactory::new(), Box::new(RejectingExecutor));
  let result = shared.execute(Box::new(|| {}));
  assert!(matches!(result, Err(ExecuteError::Rejected)));
}

#[test]
fn shutdown_invokes_inner_shutdown() {
  let count = Arc::new(AtomicUsize::new(0));
  let shared = ExecutorSharedFactory::create(
    &BuiltinSpinSharedFactory::new(),
    Box::new(CountingExecutor { count: Arc::clone(&count) }),
  );
  shared.execute(Box::new(|| {})).expect("execute should succeed");
  assert_eq!(count.load(Ordering::SeqCst), 1);
  shared.shutdown();
  assert_eq!(count.load(Ordering::SeqCst), 0);
}

#[test]
fn clone_shares_inner_state() {
  let count = Arc::new(AtomicUsize::new(0));
  let shared = ExecutorSharedFactory::create(
    &BuiltinSpinSharedFactory::new(),
    Box::new(CountingExecutor { count: Arc::clone(&count) }),
  );
  let cloned = shared.clone();
  shared.execute(Box::new(|| {})).expect("execute should succeed");
  cloned.execute(Box::new(|| {})).expect("execute should succeed");
  assert_eq!(count.load(Ordering::SeqCst), 2);
}
