//! Trait for installing actor system extensions via the builder.

use crate::core::kernel::system::{ActorSystem, ActorSystemBuildError};

/// Installs an [`Extension`](crate::core::kernel::actor::extension::Extension) during actor system
/// bootstrap.
pub trait ExtensionInstaller: Send + Sync + 'static {
  /// Invoked after the actor system has been created to register the extension.
  ///
  /// # Errors
  ///
  /// Returns an error if the extension installation fails.
  fn install(&self, system: &ActorSystem) -> Result<(), ActorSystemBuildError>;
}

impl<F> ExtensionInstaller for F
where
  F: Fn(&ActorSystem) -> Result<(), ActorSystemBuildError> + Send + Sync + 'static,
{
  fn install(&self, system: &ActorSystem) -> Result<(), ActorSystemBuildError> {
    self(system)
  }
}
