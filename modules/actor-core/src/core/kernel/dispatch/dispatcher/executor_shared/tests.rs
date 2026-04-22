use alloc::{boxed::Box, sync::Arc};
use core::sync::atomic::{AtomicUsize, Ordering};

use crate::core::kernel::dispatch::dispatcher::{ExecuteError, Executor, ExecutorShared, TrampolineState};

struct CountingExecutor {
  count: Arc<AtomicUsize>,
}

impl Executor for CountingExecutor {
  fn execute(&mut self, task: Box<dyn FnOnce() + Send + 'static>, _affinity_key: u64) -> Result<(), ExecuteError> {
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
  fn execute(&mut self, _task: Box<dyn FnOnce() + Send + 'static>, _affinity_key: u64) -> Result<(), ExecuteError> {
    Err(ExecuteError::Rejected)
  }

  fn shutdown(&mut self) {}
}

#[test]
fn execute_delegates_to_inner() {
  let count = Arc::new(AtomicUsize::new(0));
  let shared = ExecutorShared::new(Box::new(CountingExecutor { count: Arc::clone(&count) }), TrampolineState::new());
  let observed = Arc::new(AtomicUsize::new(0));
  let observed_clone = Arc::clone(&observed);
  shared
    .execute(
      Box::new(move || {
        observed_clone.store(1, Ordering::SeqCst);
      }),
      0,
    )
    .expect("execute should succeed");
  assert_eq!(count.load(Ordering::SeqCst), 1);
  assert_eq!(observed.load(Ordering::SeqCst), 1);
}

#[test]
fn execute_propagates_errors() {
  let shared = ExecutorShared::new(Box::new(RejectingExecutor), TrampolineState::new());
  let result = shared.execute(Box::new(|| {}), 0);
  assert!(matches!(result, Err(ExecuteError::Rejected)));
}

#[test]
fn shutdown_invokes_inner_shutdown() {
  let count = Arc::new(AtomicUsize::new(0));
  let shared = ExecutorShared::new(Box::new(CountingExecutor { count: Arc::clone(&count) }), TrampolineState::new());
  shared.execute(Box::new(|| {}), 0).expect("execute should succeed");
  assert_eq!(count.load(Ordering::SeqCst), 1);
  shared.shutdown();
  assert_eq!(count.load(Ordering::SeqCst), 0);
}

#[test]
fn clone_shares_inner_state() {
  let count = Arc::new(AtomicUsize::new(0));
  let shared = ExecutorShared::new(Box::new(CountingExecutor { count: Arc::clone(&count) }), TrampolineState::new());
  let cloned = shared.clone();
  shared.execute(Box::new(|| {}), 0).expect("execute should succeed");
  cloned.execute(Box::new(|| {}), 0).expect("execute should succeed");
  assert_eq!(count.load(Ordering::SeqCst), 2);
}

#[test]
fn enter_drive_guard_claims_running_slot_via_cas() {
  let count = Arc::new(AtomicUsize::new(0));
  let shared = ExecutorShared::new(Box::new(CountingExecutor { count: Arc::clone(&count) }), TrampolineState::new());

  let token = shared.enter_drive_guard();
  assert!(token.claimed(), "first enter should claim the running slot");
}

#[test]
fn enter_drive_guard_is_no_op_when_already_claimed() {
  let count = Arc::new(AtomicUsize::new(0));
  let shared = ExecutorShared::new(Box::new(CountingExecutor { count: Arc::clone(&count) }), TrampolineState::new());

  let outer = shared.enter_drive_guard();
  assert!(outer.claimed());
  let inner = shared.enter_drive_guard();
  assert!(!inner.claimed(), "nested enter must not re-claim the running slot");

  drop(inner);
  // outer still holds the claim — no new claim possible yet
  let concurrent = shared.enter_drive_guard();
  assert!(!concurrent.claimed(), "outer guard still owns the running slot after inner dropped");
}

#[test]
fn drive_guard_token_release_allows_subsequent_claim() {
  let count = Arc::new(AtomicUsize::new(0));
  let shared = ExecutorShared::new(Box::new(CountingExecutor { count: Arc::clone(&count) }), TrampolineState::new());

  {
    let token = shared.enter_drive_guard();
    assert!(token.claimed());
  }
  // previous token dropped → running slot is free
  let retry = shared.enter_drive_guard();
  assert!(retry.claimed(), "running slot should be reclaimable after the first token drops");
}

#[test]
fn execute_inside_drive_guard_enqueues_without_calling_inner() {
  let count = Arc::new(AtomicUsize::new(0));
  let shared = ExecutorShared::new(Box::new(CountingExecutor { count: Arc::clone(&count) }), TrampolineState::new());

  let token = shared.enter_drive_guard();
  assert!(token.claimed());

  // Submit a task while the guard holds running=true. The task should queue
  // into the trampoline but NOT reach the inner executor yet.
  shared.execute(Box::new(|| {}), 0).expect("execute should succeed");
  assert_eq!(count.load(Ordering::SeqCst), 0, "inner.execute must not run while the guard is held");

  // Dropping the token releases running=false but does NOT tail-drain. The
  // task stays in the trampoline queue until the next external execute.
  drop(token);
  assert_eq!(count.load(Ordering::SeqCst), 0, "DriveGuardToken::drop must not tail-drain the trampoline");

  // The next external execute picks up both the leftover task and the new one.
  shared.execute(Box::new(|| {}), 0).expect("execute should succeed");
  assert_eq!(count.load(Ordering::SeqCst), 2, "pending task + new task should both drain");
}

#[test]
fn execute_without_drive_guard_drains_as_before() {
  let count = Arc::new(AtomicUsize::new(0));
  let shared = ExecutorShared::new(Box::new(CountingExecutor { count: Arc::clone(&count) }), TrampolineState::new());

  // Baseline: without ever calling enter_drive_guard, execute behaves
  // exactly like before — each call drains the task.
  shared.execute(Box::new(|| {}), 0).expect("execute should succeed");
  assert_eq!(count.load(Ordering::SeqCst), 1);
  shared.execute(Box::new(|| {}), 0).expect("execute should succeed");
  assert_eq!(count.load(Ordering::SeqCst), 2);
}
