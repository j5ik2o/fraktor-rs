use fraktor_utils_rs::core::sync::ArcShared;

use super::dispatcher_settings::DispatcherSettings;
use crate::core::kernel::{
  actor::spawn::SpawnError,
  dispatch::{dispatcher::DispatcherShared, mailbox::Mailbox},
};

/// Provisioned dispatcher instance for a single actor bootstrap.
pub trait Dispatcher: Send + Sync {
  /// Returns the immutable settings snapshot captured for this dispatcher.
  fn settings(&self) -> &DispatcherSettings;

  /// Builds the runtime dispatcher bound to the provided mailbox.
  ///
  /// # Errors
  ///
  /// Returns [`SpawnError`] when the dispatcher cannot be provisioned for the
  /// mailbox.
  fn build_dispatcher(&self, mailbox: ArcShared<Mailbox>) -> Result<DispatcherShared, SpawnError>;
}
