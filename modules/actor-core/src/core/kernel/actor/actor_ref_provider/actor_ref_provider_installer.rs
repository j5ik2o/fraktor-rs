//! Trait for configuring actor-ref providers via the builder.

use crate::core::kernel::system::{ActorSystem, ActorSystemBuildError};

/// Installs a custom actor-ref provider after the actor system boots.
pub trait ActorRefProviderInstaller: Send + Sync + 'static {
  /// Installs the provider.
  ///
  /// # Errors
  ///
  /// Returns an error if the provider installation fails.
  fn install(&self, system: &ActorSystem) -> Result<(), ActorSystemBuildError>;
}

impl<F> ActorRefProviderInstaller for F
where
  F: Fn(&ActorSystem) -> Result<(), ActorSystemBuildError> + Send + Sync + 'static,
{
  fn install(&self, system: &ActorSystem) -> Result<(), ActorSystemBuildError> {
    self(system)
  }
}
