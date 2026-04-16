extern crate std;

use alloc::{boxed::Box, sync::Arc};
use core::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc;

use fraktor_actor_core_rs::core::kernel::dispatch::dispatcher::{ExecuteError, Executor};

use super::AffinityExecutor;

#[test]
fn tasks_execute_on_worker_threads() {
  let mut executor = AffinityExecutor::new("affinity-test", 4, 64);
  let count = Arc::new(AtomicUsize::new(0));
  let (tx, rx) = mpsc::channel();

  for _ in 0..4 {
    let c = Arc::clone(&count);
    let done = tx.clone();
    executor
      .execute(Box::new(move || {
        c.fetch_add(1, Ordering::SeqCst);
        let _ = done.send(());
      }))
      .expect("submit must succeed");
  }

  for _ in 0..4 {
    rx.recv().expect("task must complete");
  }
  assert_eq!(count.load(Ordering::SeqCst), 4);
  executor.shutdown();
}

#[test]
fn execute_after_shutdown_returns_error() {
  let mut executor = AffinityExecutor::new("affinity-shutdown", 2, 16);
  executor.shutdown();
  let result = executor.execute(Box::new(|| {}));
  assert!(matches!(result, Err(ExecuteError::Shutdown)));
}

#[test]
fn parallelism_returns_worker_count() {
  let executor = AffinityExecutor::new("affinity-par", 8, 32);
  assert_eq!(executor.parallelism(), 8);
}

#[test]
fn shutdown_is_idempotent() {
  let mut executor = AffinityExecutor::new("affinity-idem", 2, 16);
  executor.shutdown();
  executor.shutdown(); // second call must not panic
}

#[test]
fn rejected_when_queue_full() {
  let mut executor = AffinityExecutor::new("affinity-reject", 1, 1);
  let (hold_tx, hold_rx) = mpsc::channel::<()>();
  let (ack_tx, ack_rx) = mpsc::channel();

  // Submit a task that blocks the worker until we signal.
  let ack = ack_tx.clone();
  executor
    .execute(Box::new(move || {
      let _ = ack.send(());
      let _ = hold_rx.recv(); // block until main releases
    }))
    .expect("first submission");

  // Wait for the blocking task to start executing.
  ack_rx.recv().expect("task must start");

  // Now the worker is blocked. With queue_capacity=1, submissions should
  // eventually fail with Rejected.
  let mut saw_rejected = false;
  for _ in 0..8 {
    match executor.execute(Box::new(|| {})) {
      | Ok(()) => {},
      | Err(ExecuteError::Rejected) => {
        saw_rejected = true;
        break;
      },
      | Err(other) => panic!("unexpected error: {other}"),
    }
  }

  // Release the blocking task.
  drop(hold_tx);

  assert!(saw_rejected, "expected at least one Rejected error from bounded queue");

  executor.shutdown();
}
