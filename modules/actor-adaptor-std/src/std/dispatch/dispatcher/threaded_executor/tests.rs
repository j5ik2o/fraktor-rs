extern crate std;

use alloc::{boxed::Box, sync::Arc};
use core::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Barrier, mpsc};

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
    .execute(
      Box::new(move || {
        count_clone.fetch_add(1, Ordering::SeqCst);
        barrier_clone.wait();
      }),
      0,
    )
    .expect("execute should succeed");
  barrier.wait();
  assert_eq!(count.load(Ordering::SeqCst), 1);
}

#[test]
fn with_name_assigns_spawned_thread_name() {
  let mut executor = ThreadedExecutor::with_name("actor-blocking");
  let (tx, rx) = mpsc::channel();

  executor
    .execute(
      Box::new(move || {
        let name = std::thread::current().name().map(ToOwned::to_owned);
        tx.send(name).expect("send thread name");
      }),
      0,
    )
    .expect("execute should succeed");

  assert_eq!(rx.recv().expect("recv thread name").as_deref(), Some("actor-blocking"));
}

#[test]
fn shutdown_is_noop_and_later_execute_still_runs() {
  let mut executor = ThreadedExecutor::new();
  executor.shutdown();

  // ThreadedExecutor starts a fresh OS thread per task, so shutdown has no
  // worker queue to close and intentionally does not reject later execution.
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
    .expect("execute after shutdown should still be accepted");

  barrier.wait();
  assert_eq!(count.load(Ordering::SeqCst), 1);
}
