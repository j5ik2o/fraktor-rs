extern crate alloc;

use alloc::boxed::Box;

use fraktor_actor_core_rs::core::kernel::{
  actor::spawn::SpawnError,
  dispatch::dispatcher::{
    ConfiguredDispatcherBuilder, DispatcherBuilder, DispatcherProvider, DispatcherProvisionRequest,
    DispatcherRegistryEntry, DispatcherSettings, ScheduleAdapterShared,
  },
};
use tokio::runtime::Handle;

use super::{StdScheduleAdapter, dispatch_executor::TokioExecutor};

/// Default std runtime dispatcher policy backed by the current Tokio handle.
#[derive(Clone)]
pub struct DefaultDispatcherProvider {
  handle: Handle,
}

impl DefaultDispatcherProvider {
  /// Creates the default dispatcher policy from the current Tokio runtime.
  ///
  /// # Panics
  ///
  /// Panics when called outside a Tokio runtime context.
  #[must_use]
  pub fn new() -> Self {
    match Self::try_new() {
      | Some(dispatcher) => dispatcher,
      | None => panic!("Tokio runtime handle unavailable"),
    }
  }

  /// Tries to create the default dispatcher policy from the current Tokio runtime.
  #[must_use]
  pub fn try_new() -> Option<Self> {
    Handle::try_current().ok().map(|handle| Self { handle })
  }

  /// Converts this policy into a registry entry with std schedule settings.
  #[must_use]
  pub fn into_entry(self) -> DispatcherRegistryEntry {
    let settings = DispatcherSettings::default()
      .with_schedule_adapter(ScheduleAdapterShared::new(Box::new(StdScheduleAdapter::default())));
    DispatcherRegistryEntry::new(self, settings)
  }
}

impl Default for DefaultDispatcherProvider {
  fn default() -> Self {
    Self::new()
  }
}

impl DispatcherProvider for DefaultDispatcherProvider {
  fn provision(
    &self,
    settings: &DispatcherSettings,
    _request: &DispatcherProvisionRequest,
  ) -> Result<Box<dyn DispatcherBuilder>, SpawnError> {
    Ok(Box::new(ConfiguredDispatcherBuilder::from_executor_with_settings(
      Box::new(TokioExecutor::new(self.handle.clone())),
      settings.clone(),
    )))
  }
}
