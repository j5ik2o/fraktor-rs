//! Generic setup wrapper for installing typed extensions during bootstrap.

#[cfg(test)]
mod tests;

use fraktor_utils_core_rs::core::sync::ArcShared;

use crate::core::kernel::{
  actor::extension::{ExtensionId, ExtensionInstaller, install_extension_id},
  system::{ActorSystem, ActorSystemBuildError},
};

type ExtensionFactory<I> = dyn Fn(&ActorSystem) -> <I as ExtensionId>::Ext + Send + Sync + 'static;

/// Binds an [`ExtensionId`] to a custom factory used during actor-system startup.
pub struct ExtensionSetup<I>
where
  I: ExtensionId + Clone, {
  extension_id:     I,
  create_extension: ArcShared<ExtensionFactory<I>>,
}

impl<I> ExtensionSetup<I>
where
  I: ExtensionId + Clone,
{
  /// Creates a new setup for the provided extension identifier and factory.
  #[must_use]
  pub fn new<F>(extension_id: I, create_extension: F) -> Self
  where
    F: Fn(&ActorSystem) -> I::Ext + Send + Sync + 'static, {
    Self { extension_id, create_extension: ArcShared::new(create_extension) }
  }

  /// Returns the wrapped extension identifier.
  #[must_use]
  pub const fn extension_id(&self) -> &I {
    &self.extension_id
  }
}

impl<I> Clone for ExtensionSetup<I>
where
  I: ExtensionId + Clone,
{
  fn clone(&self) -> Self {
    Self { extension_id: self.extension_id.clone(), create_extension: self.create_extension.clone() }
  }
}

impl<I> ExtensionId for ExtensionSetup<I>
where
  I: ExtensionId + Clone,
{
  type Ext = I::Ext;

  fn create_extension(&self, system: &ActorSystem) -> Self::Ext {
    (self.create_extension)(system)
  }

  fn id(&self) -> core::any::TypeId {
    self.extension_id.id()
  }
}

impl<I> ExtensionInstaller for ExtensionSetup<I>
where
  I: ExtensionId + Clone,
{
  fn install(&self, system: &ActorSystem) -> Result<(), ActorSystemBuildError> {
    install_extension_id(system, self);
    Ok(())
  }
}
