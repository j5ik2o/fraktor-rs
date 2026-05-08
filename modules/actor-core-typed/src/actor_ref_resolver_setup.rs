//! Setup wrapper for replacing the default typed actor-ref resolver extension.

use core::any::TypeId;

use fraktor_actor_core_rs::core::kernel::{
  actor::extension::{ExtensionId, ExtensionInstaller},
  system::{ActorSystem, ActorSystemBuildError},
};

use super::{ActorRefResolver, ExtensionSetup, internal::ActorRefResolverId};

/// Replaces the default [`ActorRefResolver`] extension during actor-system startup.
#[derive(Clone)]
pub struct ActorRefResolverSetup {
  inner: ExtensionSetup<ActorRefResolverId>,
}

impl ActorRefResolverSetup {
  /// Creates a new setup with a custom [`ActorRefResolver`] factory.
  #[must_use]
  pub fn new<F>(create_extension: F) -> Self
  where
    F: Fn(&ActorSystem) -> ActorRefResolver + Send + Sync + 'static, {
    Self { inner: ExtensionSetup::new(ActorRefResolverId::new(), create_extension) }
  }
}

impl ExtensionId for ActorRefResolverSetup {
  type Ext = ActorRefResolver;

  fn create_extension(&self, system: &ActorSystem) -> Self::Ext {
    self.inner.create_extension(system)
  }

  fn id(&self) -> TypeId {
    self.inner.id()
  }
}

impl ExtensionInstaller for ActorRefResolverSetup {
  fn install(&self, system: &ActorSystem) -> Result<(), ActorSystemBuildError> {
    self.inner.install(system)
  }
}
