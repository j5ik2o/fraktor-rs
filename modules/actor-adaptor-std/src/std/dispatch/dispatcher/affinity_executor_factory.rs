//! [`ExecutorFactory`] that builds an [`AffinityExecutor`] per dispatcher.

extern crate std;

use core::sync::atomic::{AtomicUsize, Ordering};
use std::string::String;

use fraktor_actor_core_rs::core::kernel::dispatch::dispatcher::{ExecutorFactory, ExecutorShared, TrampolineState};

use super::affinity_executor::AffinityExecutor;

/// Factory that produces a fresh [`AffinityExecutor`] per `create` call.
///
/// Each call increments a static counter to suffix the worker thread names so
/// the spawned threads can be identified in stack traces and metrics.
pub struct AffinityExecutorFactory {
  thread_name_prefix: String,
  parallelism:        usize,
  queue_capacity:     usize,
  counter:            AtomicUsize,
}

impl AffinityExecutorFactory {
  /// Creates a factory with the supplied thread name prefix, parallelism, and
  /// per-worker queue capacity.
  ///
  /// # Panics
  ///
  /// Panics if `parallelism` is zero.
  #[must_use]
  pub fn new(thread_name_prefix: impl Into<String>, parallelism: usize, queue_capacity: usize) -> Self {
    assert!(parallelism > 0, "parallelism must be > 0");
    Self { thread_name_prefix: thread_name_prefix.into(), parallelism, queue_capacity, counter: AtomicUsize::new(0) }
  }

  fn allocate_prefix(&self, dispatcher_id: &str) -> String {
    let index = self.counter.fetch_add(1, Ordering::SeqCst);
    alloc::format!("{}-{}-{}", self.thread_name_prefix, dispatcher_id, index)
  }
}

impl ExecutorFactory for AffinityExecutorFactory {
  fn create(&self, dispatcher_id: &str) -> ExecutorShared {
    let prefix = self.allocate_prefix(dispatcher_id);
    ExecutorShared::new(
      Box::new(AffinityExecutor::new(&prefix, self.parallelism, self.queue_capacity)),
      TrampolineState::new(),
    )
  }
}
