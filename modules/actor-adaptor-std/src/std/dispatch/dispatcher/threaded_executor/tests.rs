extern crate std;

use alloc::{boxed::Box, string::String, sync::Arc};
use core::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Barrier, Mutex};

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
  let observed = Arc::new(Mutex::new(None::<String>));
  let barrier = Arc::new(Barrier::new(2));

  let observed_clone = Arc::clone(&observed);
  let barrier_clone = Arc::clone(&barrier);
  executor
    .execute(
      Box::new(move || {
        let name = std::thread::current().name().map(ToOwned::to_owned);
        *observed_clone.lock().expect("thread name lock") = name;
        barrier_clone.wait();
      }),
      0,
    )
    .expect("execute should succeed");

  barrier.wait();
  assert_eq!(observed.lock().expect("thread name lock").as_deref(), Some("actor-blocking"));
}

#[test]
fn shutdown_is_noop_and_later_execute_still_runs() {
  let mut executor = ThreadedExecutor::new();
  executor.shutdown();

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
