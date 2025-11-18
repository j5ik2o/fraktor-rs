//! Trait for configuring actor-ref providers via the builder.

use fraktor_utils_rs::core::runtime_toolbox::RuntimeToolbox;

use crate::core::system::{ActorSystemBuildError, ActorSystemGeneric};

/// Installs a custom actor-ref provider after the actor system boots.
pub trait ActorRefProviderInstaller<TB>: Send + Sync + 'static
where
  TB: RuntimeToolbox + 'static, {
  /// Installs the provider.
  fn install(&self, system: &ActorSystemGeneric<TB>) -> Result<(), ActorSystemBuildError>;
}

impl<TB, F> ActorRefProviderInstaller<TB> for F
where
  TB: RuntimeToolbox + 'static,
  F: Fn(&ActorSystemGeneric<TB>) -> Result<(), ActorSystemBuildError> + Send + Sync + 'static,
{
  fn install(&self, system: &ActorSystemGeneric<TB>) -> Result<(), ActorSystemBuildError> {
    self(system)
  }
}
