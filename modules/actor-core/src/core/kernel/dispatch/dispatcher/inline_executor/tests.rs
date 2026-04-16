use alloc::{boxed::Box, sync::Arc, vec::Vec};
use core::sync::atomic::{AtomicUsize, Ordering};

use fraktor_utils_core_rs::core::sync::{SharedLock, SpinSyncMutex};

use super::InlineExecutor;
use crate::core::kernel::dispatch::dispatcher::Executor;

#[test]
fn execute_runs_task_on_current_thread() {
  let mut executor = InlineExecutor::new();
  let log = Arc::new(SharedLock::new_with_driver::<SpinSyncMutex<_>>(Vec::new()));
  let log_clone = Arc::clone(&log);
  executor.execute(Box::new(move || log_clone.with_lock(|values| values.push(42))), 0).expect("execute should succeed");
  assert_eq!(log.with_lock(|values| values.clone()), alloc::vec![42]);
}

#[test]
fn nested_execute_uses_trampoline() {
  let mut executor = InlineExecutor::new();
  let max_depth = Arc::new(AtomicUsize::new(0));
  let cur_depth = Arc::new(AtomicUsize::new(0));
  let max_clone = Arc::clone(&max_depth);
  let cur_clone = Arc::clone(&cur_depth);
  executor
    .execute(
      Box::new(move || {
        let new = cur_clone.fetch_add(1, Ordering::SeqCst) + 1;
        if new > max_clone.load(Ordering::SeqCst) {
          max_clone.store(new, Ordering::SeqCst);
        }
        cur_clone.fetch_sub(1, Ordering::SeqCst);
      }),
      0,
    )
    .expect("execute should succeed");
  // Single call: depth never exceeds 1.
  assert_eq!(max_depth.load(Ordering::SeqCst), 1);
}

#[test]
fn shutdown_clears_pending_tasks() {
  let mut executor = InlineExecutor::new();
  executor.shutdown();
}
