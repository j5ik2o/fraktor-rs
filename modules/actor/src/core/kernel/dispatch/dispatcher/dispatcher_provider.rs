use alloc::boxed::Box;

use super::{
  dispatcher_provision_request::DispatcherProvisionRequest, dispatcher_settings::DispatcherSettings,
  dispatcher_trait::Dispatcher,
};
use crate::core::kernel::actor::spawn::SpawnError;

/// Provider that provisions actor-specific dispatcher instances from settings.
pub trait DispatcherProvider: Send + Sync {
  /// Provisions a dispatcher for a single actor bootstrap request.
  ///
  /// # Errors
  ///
  /// Returns [`SpawnError`] when the provider cannot provision a dispatcher for
  /// the request.
  fn provision(
    &self,
    settings: &DispatcherSettings,
    request: &DispatcherProvisionRequest,
  ) -> Result<Box<dyn Dispatcher>, SpawnError>;
}
