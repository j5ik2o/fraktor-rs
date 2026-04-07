use alloc::{boxed::Box, sync::Arc};
use core::sync::atomic::{AtomicUsize, Ordering};

use fraktor_actor_core_rs::core::kernel::dispatch::dispatcher::Executor;

use super::TokioExecutor;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn execute_runs_task_via_blocking_pool() {
  let count = Arc::new(AtomicUsize::new(0));
  let mut executor = TokioExecutor::new(tokio::runtime::Handle::current());
  let count_clone = Arc::clone(&count);
  let waited = Arc::new(tokio::sync::Notify::new());
  let waited_clone = Arc::clone(&waited);
  executor
    .execute(Box::new(move || {
      count_clone.fetch_add(1, Ordering::SeqCst);
      waited_clone.notify_one();
    }))
    .expect("execute should succeed");
  waited.notified().await;
  assert_eq!(count.load(Ordering::SeqCst), 1);
}

#[test]
fn supports_blocking_returns_true() {
  let runtime = tokio::runtime::Builder::new_current_thread().build().expect("runtime");
  let executor = TokioExecutor::new(runtime.handle().clone());
  assert!(executor.supports_blocking());
}
