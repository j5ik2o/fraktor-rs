//! Setup wrapper for replacing the default typed receptionist extension.

#[cfg(test)]
#[path = "receptionist_setup_test.rs"]
mod tests;

use core::any::TypeId;

use fraktor_actor_core_kernel_rs::{
  actor::extension::{ExtensionId, ExtensionInstaller},
  system::{ActorSystem, ActorSystemBuildError},
};

use super::{Receptionist, extension::ReceptionistExtensionId};
use crate::ExtensionSetup;

/// Replaces the default [`Receptionist`] extension during actor-system startup.
#[derive(Clone)]
pub struct ReceptionistSetup {
  inner: ExtensionSetup<ReceptionistExtensionId>,
}

impl ReceptionistSetup {
  /// Creates a new setup with a custom [`Receptionist`] factory.
  #[must_use]
  pub fn new<F>(create_extension: F) -> Self
  where
    F: Fn(&ActorSystem) -> Receptionist + Send + Sync + 'static, {
    Self { inner: ExtensionSetup::new(ReceptionistExtensionId::new(), create_extension) }
  }
}

impl ExtensionId for ReceptionistSetup {
  type Ext = Receptionist;

  fn create_extension(&self, system: &ActorSystem) -> Self::Ext {
    self.inner.create_extension(system)
  }

  fn id(&self) -> TypeId {
    self.inner.id()
  }
}

impl ExtensionInstaller for ReceptionistSetup {
  fn install(&self, system: &ActorSystem) -> Result<(), ActorSystemBuildError> {
    self.inner.install(system)
  }
}
