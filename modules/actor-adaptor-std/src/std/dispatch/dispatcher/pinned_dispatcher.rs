extern crate alloc;

use alloc::{boxed::Box, format, string::String};
use core::sync::atomic::{AtomicUsize, Ordering};

use fraktor_actor_core_rs::core::kernel::{
  actor::spawn::SpawnError,
  dispatch::dispatcher::{
    Dispatcher, DispatcherConfig, DispatcherProvider, DispatcherProvisionRequest, DispatcherRegistryEntry,
    DispatcherSettings, ScheduleAdapterShared,
  },
};

#[cfg(test)]
mod tests;

use super::{StdScheduleAdapter, pinned_executor::PinnedExecutor};

/// Counter for generating unique thread names across all pinned dispatchers.
static PINNED_THREAD_COUNTER: AtomicUsize = AtomicUsize::new(0);

/// Dispatcher policy that dedicates a single execution lane to each actor.
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

  /// Converts this policy into a registry entry with std schedule settings.
  #[must_use]
  pub fn into_entry(self) -> DispatcherRegistryEntry {
    let settings = DispatcherSettings::default()
      .with_schedule_adapter(ScheduleAdapterShared::new(Box::new(StdScheduleAdapter::default())));
    DispatcherRegistryEntry::new(self, settings)
  }
}

impl Default for PinnedDispatcher {
  fn default() -> Self {
    Self::new()
  }
}

impl DispatcherProvider for PinnedDispatcher {
  fn provision(
    &self,
    settings: &DispatcherSettings,
    request: &DispatcherProvisionRequest,
  ) -> Result<Box<dyn Dispatcher>, SpawnError> {
    let seq = PINNED_THREAD_COUNTER.fetch_add(1, Ordering::Relaxed);
    let actor_name = request.actor_name().unwrap_or(request.dispatcher_id());
    let name = format!("{}-{}-{seq}", self.thread_name_prefix, actor_name);
    Ok(Box::new(DispatcherConfig::from_executor_with_settings(
      Box::new(PinnedExecutor::with_name(name)),
      settings.clone(),
    )))
  }
}
