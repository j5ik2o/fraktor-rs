//! Trait for installing actor system extensions via the builder.

use fraktor_utils_rs::core::runtime_toolbox::RuntimeToolbox;

use crate::core::system::{ActorSystemBuildError, ActorSystemGeneric};

/// Installs an [`Extension`] during actor system bootstrap.
pub trait ExtensionInstaller<TB>: Send + Sync + 'static
where
  TB: RuntimeToolbox + 'static, {
  /// Invoked after the actor system has been created to register the extension.
  ///
  /// # Errors
  ///
  /// Returns an error if the extension installation fails.
  fn install(&self, system: &ActorSystemGeneric<TB>) -> Result<(), ActorSystemBuildError>;
}

impl<TB, F> ExtensionInstaller<TB> for F
where
  TB: RuntimeToolbox + 'static,
  F: Fn(&ActorSystemGeneric<TB>) -> Result<(), ActorSystemBuildError> + Send + Sync + 'static,
{
  fn install(&self, system: &ActorSystemGeneric<TB>) -> Result<(), ActorSystemBuildError> {
    self(system)
  }
}
