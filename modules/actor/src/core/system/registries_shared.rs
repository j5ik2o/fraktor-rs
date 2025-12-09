//! Shared wrapper for name registries collection.

use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdToolbox, RuntimeToolbox, SyncMutexFamily, ToolboxMutex},
  sync::{ArcShared, SharedAccess},
};

use super::registries::RegistriesGeneric;

/// Shared wrapper for [`RegistriesGeneric`].
///
/// This wrapper provides [`SharedAccess`] methods (`with_read`/`with_write`)
/// that internally lock the underlying collection, allowing safe
/// concurrent access from multiple owners.
pub(crate) struct RegistriesSharedGeneric<TB: RuntimeToolbox + 'static> {
  inner: ArcShared<ToolboxMutex<RegistriesGeneric<TB>, TB>>,
}

/// Type alias using the default toolbox.
#[allow(dead_code)]
pub(crate) type RegistriesShared = RegistriesSharedGeneric<NoStdToolbox>;

impl<TB: RuntimeToolbox + 'static> RegistriesSharedGeneric<TB> {
  /// Creates a new shared wrapper around the provided registries collection.
  #[must_use]
  pub(crate) fn new(registries: RegistriesGeneric<TB>) -> Self {
    Self { inner: ArcShared::new(<TB::MutexFamily as SyncMutexFamily>::create(registries)) }
  }
}

impl<TB: RuntimeToolbox + 'static> Default for RegistriesSharedGeneric<TB> {
  fn default() -> Self {
    Self::new(RegistriesGeneric::default())
  }
}

impl<TB: RuntimeToolbox> Clone for RegistriesSharedGeneric<TB> {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}

impl<TB: RuntimeToolbox + 'static> SharedAccess<RegistriesGeneric<TB>> for RegistriesSharedGeneric<TB> {
  fn with_read<R>(&self, f: impl FnOnce(&RegistriesGeneric<TB>) -> R) -> R {
    self.inner.with_read(f)
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut RegistriesGeneric<TB>) -> R) -> R {
    self.inner.with_write(f)
  }
}
