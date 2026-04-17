use alloc::{boxed::Box, sync::Arc};
use core::sync::atomic::{AtomicUsize, Ordering};

use fraktor_actor_core_rs::core::kernel::dispatch::dispatcher::Executor;
use tokio::{runtime::Handle, sync::Notify};

use super::TokioExecutor;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn execute_runs_task_via_blocking_pool() {
  let count = Arc::new(AtomicUsize::new(0));
  let mut executor = TokioExecutor::new(Handle::current());
  let count_clone = Arc::clone(&count);
  let waited = Arc::new(Notify::new());
  let waited_clone = Arc::clone(&waited);
  executor
    .execute(
      Box::new(move || {
        count_clone.fetch_add(1, Ordering::SeqCst);
        waited_clone.notify_one();
      }),
      0,
    )
    .expect("execute should succeed");
  waited.notified().await;
  assert_eq!(count.load(Ordering::SeqCst), 1);
}
