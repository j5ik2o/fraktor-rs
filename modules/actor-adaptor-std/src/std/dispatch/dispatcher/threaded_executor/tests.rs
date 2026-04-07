extern crate std;

use alloc::{boxed::Box, sync::Arc};
use core::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Barrier;

use fraktor_actor_core_rs::core::kernel::dispatch::dispatcher::Executor;

use super::ThreadedExecutor;

#[test]
fn execute_runs_each_task_on_a_new_thread() {
  let mut executor = ThreadedExecutor::new();
  let count = Arc::new(AtomicUsize::new(0));
  let barrier = Arc::new(Barrier::new(2));

  let count_clone = Arc::clone(&count);
  let barrier_clone = Arc::clone(&barrier);
  executor
    .execute(Box::new(move || {
      count_clone.fetch_add(1, Ordering::SeqCst);
      barrier_clone.wait();
    }))
    .expect("execute should succeed");
  barrier.wait();
  assert_eq!(count.load(Ordering::SeqCst), 1);
}
