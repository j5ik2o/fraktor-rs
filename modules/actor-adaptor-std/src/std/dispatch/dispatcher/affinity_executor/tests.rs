extern crate std;

use alloc::{boxed::Box, sync::Arc, vec::Vec};
use core::sync::atomic::{AtomicUsize, Ordering};
use std::{
  collections::HashSet,
  sync::{Mutex, mpsc},
  thread::{self, ThreadId},
};

use fraktor_actor_core_rs::dispatch::dispatcher::{ExecuteError, Executor};

use super::AffinityExecutor;

#[test]
fn tasks_execute_on_worker_threads() {
  let mut executor = AffinityExecutor::new("affinity-test", 4, 64);
  let count = Arc::new(AtomicUsize::new(0));
  let (tx, rx) = mpsc::channel();

  for i in 0..4_u64 {
    let c = Arc::clone(&count);
    let done = tx.clone();
    executor
      .execute(
        Box::new(move || {
          c.fetch_add(1, Ordering::SeqCst);
          let _ = done.send(());
        }),
        i,
      )
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
  let result = executor.execute(Box::new(|| {}), 0);
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
    .execute(
      Box::new(move || {
        let _ = ack.send(());
        let _ = hold_rx.recv(); // block until main releases
      }),
      0,
    )
    .expect("first submission");

  // Wait for the blocking task to start executing.
  ack_rx.recv().expect("task must start");

  // Now the worker is blocked. With queue_capacity=1, submissions should
  // eventually fail with Rejected.
  let mut saw_rejected = false;
  for _ in 0..8 {
    match executor.execute(Box::new(|| {}), 0) {
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

#[test]
fn same_affinity_key_routes_to_same_worker() {
  let mut executor = AffinityExecutor::new("affinity-sticky", 4, 64);
  let thread_ids: Arc<Mutex<Vec<ThreadId>>> = Arc::new(Mutex::new(Vec::new()));
  let (tx, rx) = mpsc::channel();

  // Submit 8 tasks all with the same affinity key; they must land on the same worker.
  for _ in 0..8 {
    let ids = Arc::clone(&thread_ids);
    let done = tx.clone();
    executor
      .execute(
        Box::new(move || {
          ids.lock().expect("lock").push(thread::current().id());
          let _ = done.send(());
        }),
        42,
      )
      .expect("submit must succeed");
  }

  for _ in 0..8 {
    rx.recv().expect("task must complete");
  }

  let ids = thread_ids.lock().expect("lock");
  let first = ids[0];
  assert!(ids.iter().all(|id| *id == first), "all tasks with the same affinity key must run on the same thread");
  executor.shutdown();
}

#[test]
fn different_affinity_keys_distribute_across_workers() {
  let mut executor = AffinityExecutor::new("affinity-dist", 4, 64);
  let thread_ids: Arc<Mutex<Vec<(u64, ThreadId)>>> = Arc::new(Mutex::new(Vec::new()));
  let (tx, rx) = mpsc::channel();

  // Submit tasks with 4 different affinity keys (0..4).
  for key in 0..4_u64 {
    let ids = Arc::clone(&thread_ids);
    let done = tx.clone();
    executor
      .execute(
        Box::new(move || {
          ids.lock().expect("lock").push((key, thread::current().id()));
          let _ = done.send(());
        }),
        key,
      )
      .expect("submit must succeed");
  }

  for _ in 0..4 {
    rx.recv().expect("task must complete");
  }

  let ids = thread_ids.lock().expect("lock");
  let unique_threads: HashSet<_> = ids.iter().map(|(_, tid)| *tid).collect();
  // With 4 workers and keys 0..4, each key maps to a different worker.
  assert_eq!(unique_threads.len(), 4, "4 distinct keys with 4 workers should use all 4 workers");
  executor.shutdown();
}
