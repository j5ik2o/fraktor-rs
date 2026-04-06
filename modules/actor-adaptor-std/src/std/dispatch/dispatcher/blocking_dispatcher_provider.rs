extern crate alloc;

use alloc::boxed::Box;

use fraktor_actor_core_rs::core::kernel::{
  actor::spawn::SpawnError,
  dispatch::dispatcher::{
    ConfiguredDispatcherBuilder, DispatcherBuilder, DispatcherProvider, DispatcherProvisionRequest,
    DispatcherRegistryEntry, DispatcherSettings, ScheduleAdapterShared,
  },
};

use super::{StdScheduleAdapter, dispatch_executor::ThreadedExecutor};

/// Blocking-friendly dispatcher policy for std runtimes.
pub struct BlockingDispatcherProvider;

impl BlockingDispatcherProvider {
  /// Creates the blocking dispatcher policy.
  #[must_use]
  pub const fn new() -> Self {
    Self
  }

  /// Converts this policy into a registry entry with std schedule settings.
  #[must_use]
  pub fn into_entry(self) -> DispatcherRegistryEntry {
    let settings = DispatcherSettings::default()
      .with_schedule_adapter(ScheduleAdapterShared::new(Box::new(StdScheduleAdapter::default())));
    DispatcherRegistryEntry::new(self, settings)
  }
}

impl Default for BlockingDispatcherProvider {
  fn default() -> Self {
    Self::new()
  }
}

impl DispatcherProvider for BlockingDispatcherProvider {
  fn provision(
    &self,
    settings: &DispatcherSettings,
    _request: &DispatcherProvisionRequest,
  ) -> Result<Box<dyn DispatcherBuilder>, SpawnError> {
    Ok(Box::new(ConfiguredDispatcherBuilder::from_executor_with_settings(
      Box::new(ThreadedExecutor::new()),
      settings.clone(),
    )))
  }
}
