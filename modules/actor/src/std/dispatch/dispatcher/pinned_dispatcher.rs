extern crate alloc;

use alloc::{boxed::Box, format, string::String};
use core::sync::atomic::{AtomicUsize, Ordering};

use fraktor_utils_rs::{core::sync::ArcShared, std::StdSyncMutex};

use super::{DispatcherConfig, dispatch_executor::ThreadedExecutor};

#[cfg(test)]
mod tests;

/// Counter for generating unique thread names across all pinned dispatchers.
static PINNED_THREAD_COUNTER: AtomicUsize = AtomicUsize::new(0);

/// Dispatcher factory that dedicates a single execution lane to each actor.
///
/// Equivalent to Pekko's `PinnedDispatcher`.  Each call to [`build_config`]
/// creates an independent [`DispatcherConfig`] backed by its own
/// [`ThreadedExecutor`], ensuring the actor's message processing is never
/// interleaved with other actors on the same thread.
///
/// [`build_config`]: PinnedDispatcher::build_config
pub struct PinnedDispatcher {
  thread_name_prefix: String,
}

impl PinnedDispatcher {
  /// Default thread-name prefix used when none is specified.
  const DEFAULT_PREFIX: &'static str = "fraktor-pinned";

  /// Creates a pinned dispatcher factory with the default thread-name prefix.
  #[must_use]
  pub fn new() -> Self {
    Self { thread_name_prefix: String::from(Self::DEFAULT_PREFIX) }
  }

  /// Creates a pinned dispatcher factory with a custom thread-name prefix.
  ///
  /// Each dedicated thread will be named `"{prefix}-{sequence}"`.
  #[must_use]
  pub fn with_thread_name_prefix(prefix: impl Into<String>) -> Self {
    Self { thread_name_prefix: prefix.into() }
  }

  /// Returns the configured thread-name prefix.
  #[must_use]
  pub fn thread_name_prefix(&self) -> &str {
    &self.thread_name_prefix
  }

  /// Creates a new [`DispatcherConfig`] with a dedicated executor.
  ///
  /// Every invocation produces an independent execution lane.  Two configs
  /// returned from this method will never share a thread.
  #[must_use]
  pub fn build_config(&self) -> DispatcherConfig {
    let seq = PINNED_THREAD_COUNTER.fetch_add(1, Ordering::Relaxed);
    let name = format!("{}-{}", self.thread_name_prefix, seq);
    let executor = ThreadedExecutor::with_name(name);
    DispatcherConfig::from_executor(ArcShared::new(StdSyncMutex::new(Box::new(executor))))
  }
}

impl Default for PinnedDispatcher {
  fn default() -> Self {
    Self::new()
  }
}
