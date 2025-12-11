//! Shared wrapper for extensions registry.

use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdToolbox, RuntimeToolbox, SyncRwLockFamily, ToolboxRwLock},
  sync::{ArcShared, SharedAccess, sync_rwlock_like::SyncRwLockLike},
};

use super::extensions::ExtensionsGeneric;

/// Shared wrapper for [`ExtensionsGeneric`].
///
/// This wrapper provides [`SharedAccess`] methods (`with_read`/`with_write`)
/// that internally lock the underlying registry, allowing safe
/// concurrent access from multiple owners.
pub(crate) struct ExtensionsSharedGeneric<TB: RuntimeToolbox + 'static> {
  inner: ArcShared<ToolboxRwLock<ExtensionsGeneric<TB>, TB>>,
}

/// Type alias using the default toolbox.
#[allow(dead_code)]
pub(crate) type ExtensionsShared = ExtensionsSharedGeneric<NoStdToolbox>;

impl<TB: RuntimeToolbox + 'static> ExtensionsSharedGeneric<TB> {
  /// Creates a new shared wrapper around the provided extensions registry.
  #[must_use]
  pub(crate) fn new(extensions: ExtensionsGeneric<TB>) -> Self {
    Self { inner: ArcShared::new(<TB::RwLockFamily as SyncRwLockFamily>::create(extensions)) }
  }
}

impl<TB: RuntimeToolbox + 'static> Default for ExtensionsSharedGeneric<TB> {
  fn default() -> Self {
    Self::new(ExtensionsGeneric::default())
  }
}

impl<TB: RuntimeToolbox> Clone for ExtensionsSharedGeneric<TB> {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}

impl<TB: RuntimeToolbox + 'static> SharedAccess<ExtensionsGeneric<TB>> for ExtensionsSharedGeneric<TB> {
  fn with_read<R>(&self, f: impl FnOnce(&ExtensionsGeneric<TB>) -> R) -> R {
    let guard = self.inner.read();
    f(&guard)
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut ExtensionsGeneric<TB>) -> R) -> R {
    let mut guard = self.inner.write();
    f(&mut guard)
  }
}
