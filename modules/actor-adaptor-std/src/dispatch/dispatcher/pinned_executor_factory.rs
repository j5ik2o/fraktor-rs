//! [`ExecutorFactory`] that builds a [`PinnedExecutor`] per dispatcher.

extern crate std;

use core::sync::atomic::{AtomicUsize, Ordering};
use std::string::String;

use fraktor_actor_core_kernel_rs::dispatch::dispatcher::{ExecutorFactory, ExecutorShared, TrampolineState};

use super::pinned_executor::PinnedExecutor;

/// Factory that produces a fresh [`PinnedExecutor`] per `create` call.
///
/// Each call increments a static counter to suffix the worker thread name so
/// the spawned threads can be identified in stack traces and metrics.
pub struct PinnedExecutorFactory {
  thread_name_prefix: String,
  counter:            AtomicUsize,
}

impl PinnedExecutorFactory {
  /// Creates a factory using the supplied thread name prefix.
  #[must_use]
  pub fn new(thread_name_prefix: impl Into<String>) -> Self {
    Self { thread_name_prefix: thread_name_prefix.into(), counter: AtomicUsize::new(0) }
  }

  fn allocate_name(&self, dispatcher_id: &str) -> String {
    let index = self.counter.fetch_add(1, Ordering::SeqCst);
    alloc::format!("{}-{}-{}", self.thread_name_prefix, dispatcher_id, index)
  }
}

impl ExecutorFactory for PinnedExecutorFactory {
  fn create(&self, dispatcher_id: &str) -> ExecutorShared {
    let name = self.allocate_name(dispatcher_id);
    ExecutorShared::new(Box::new(PinnedExecutor::with_name(name)), TrampolineState::new())
  }
}
