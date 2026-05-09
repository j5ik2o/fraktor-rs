//! Thread-affinity [`Executor`] that pins tasks to dedicated worker threads.
//!
//! Pekko equivalent: `org.apache.pekko.dispatch.affinity.AffinityPool`.
//!
//! Each worker thread owns an exclusive bounded queue. Tasks are routed to a
//! specific worker via the `queue_selector` closure, which maps a task hash
//! (typically `mailbox_hash % parallelism`) to a queue index. This guarantees
//! that tasks belonging to the same mailbox always land on the same worker
//! thread, improving CPU cache locality.

#[cfg(test)]
mod tests;

extern crate std;

use alloc::{boxed::Box, sync::Arc, vec::Vec};
use core::sync::atomic::{AtomicU8, Ordering};
use std::{
  sync::mpsc::{SyncSender, TrySendError, sync_channel},
  thread::{self, Builder, JoinHandle},
};

use fraktor_actor_core_rs::dispatch::dispatcher::{ExecuteError, Executor};

type Task = Box<dyn FnOnce() + Send + 'static>;

/// Pool state transitions: `Running → ShuttingDown → Terminated`.
const RUNNING: u8 = 0;
const SHUTTING_DOWN: u8 = 1;
const TERMINATED: u8 = 2;

/// Thread-affinity executor that distributes tasks across a fixed set of worker
/// threads using a deterministic queue-selection function.
///
/// Pekko's `AffinityPool` pins actor mailboxes to specific worker threads so
/// that the same actor's tasks always execute on the same OS thread. This
/// implementation follows the same approach: each worker thread polls its own
/// bounded channel, and the `queue_selector` closure decides which channel
/// receives each submitted task.
///
/// # Queue selection
///
/// The `queue_selector` is a `Fn(u64, usize) -> usize` where:
/// - the first argument is a **task affinity key** (typically the mailbox hash),
/// - the second argument is the **parallelism** (number of workers),
/// - the return value is the **queue index** in `0..parallelism`.
///
/// The default selector (`AffinityExecutor::new`) uses `key % parallelism`.
pub struct AffinityExecutor {
  senders: Vec<Option<SyncSender<Task>>>,
  joins:   Vec<Option<JoinHandle<()>>>,
  state:   Arc<AtomicU8>,
}

impl AffinityExecutor {
  /// Creates a new affinity executor with `parallelism` worker threads.
  ///
  /// Each worker thread's bounded queue has capacity `queue_capacity`.
  ///
  /// # Panics
  ///
  /// Panics if `parallelism` is zero or if a worker thread cannot be spawned.
  #[must_use]
  pub fn new(thread_name_prefix: &str, parallelism: usize, queue_capacity: usize) -> Self {
    assert!(parallelism > 0, "parallelism must be > 0");

    let state = Arc::new(AtomicU8::new(RUNNING));
    let mut senders = Vec::with_capacity(parallelism);
    let mut joins = Vec::with_capacity(parallelism);

    for i in 0..parallelism {
      let (tx, rx) = sync_channel::<Task>(queue_capacity);
      let name = alloc::format!("{}-{}", thread_name_prefix, i);
      let join = Builder::new()
        .name(name)
        .spawn(move || {
          while let Ok(task) = rx.recv() {
            task();
          }
        })
        .expect("affinity executor worker thread must spawn");
      senders.push(Some(tx));
      joins.push(Some(join));
    }

    Self { senders, joins, state }
  }

  /// Returns the parallelism (number of worker threads).
  #[must_use]
  pub fn parallelism(&self) -> usize {
    self.senders.len()
  }
}

impl Executor for AffinityExecutor {
  /// Submits a task to the worker thread identified by the task's affinity key.
  ///
  /// The `affinity_key` (typically the mailbox PID value) is mapped to a queue
  /// index via `key % parallelism`, guaranteeing that the same actor's tasks
  /// always execute on the same worker thread.
  ///
  /// # Errors
  ///
  /// - [`ExecuteError::Shutdown`] if the executor has been shut down.
  /// - [`ExecuteError::Rejected`] if the target queue is full.
  fn execute(&mut self, task: Task, affinity_key: u64) -> Result<(), ExecuteError> {
    if self.state.load(Ordering::Acquire) != RUNNING {
      return Err(ExecuteError::Shutdown);
    }

    let idx = (affinity_key % self.senders.len() as u64) as usize;

    let Some(sender) = self.senders[idx].as_ref() else {
      return Err(ExecuteError::Shutdown);
    };
    sender.try_send(task).map_err(|err| match err {
      | TrySendError::Full(_) => ExecuteError::Rejected,
      | TrySendError::Disconnected(_) => ExecuteError::Shutdown,
    })
  }

  fn shutdown(&mut self) {
    if self.state.compare_exchange(RUNNING, SHUTTING_DOWN, Ordering::AcqRel, Ordering::Acquire).is_err() {
      return;
    }
    // Drop all senders so worker threads exit their recv loop.
    for sender in &mut self.senders {
      sender.take();
    }
    let current = thread::current().id();
    for join in &mut self.joins {
      if let Some(handle) = join.take() {
        if handle.thread().id() == current {
          // Cannot join from the worker thread itself.
          continue;
        }
        // Best-effort join: worker panic is not recoverable, so the join
        // result is intentionally observed-and-ignored.
        drop(handle.join());
      }
    }
    self.state.store(TERMINATED, Ordering::Release);
  }
}

impl Drop for AffinityExecutor {
  fn drop(&mut self) {
    self.shutdown();
  }
}
