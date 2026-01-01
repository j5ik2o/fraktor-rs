//! Shared wrapper for persistence extension instance.

use fraktor_actor_rs::core::extension::Extension;
use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdToolbox, RuntimeToolbox, SyncMutexFamily, ToolboxMutex},
  sync::{ArcShared, SharedAccess, sync_mutex_like::SyncMutexLike},
};

use crate::core::persistence_extension::PersistenceExtensionGeneric;

/// Shared wrapper for a persistence extension instance.
pub struct PersistenceExtensionSharedGeneric<TB: RuntimeToolbox + 'static> {
  inner: ArcShared<ToolboxMutex<PersistenceExtensionGeneric<TB>, TB>>,
}

/// Type alias using the default toolbox.
pub type PersistenceExtensionShared = PersistenceExtensionSharedGeneric<NoStdToolbox>;

impl<TB: RuntimeToolbox + 'static> PersistenceExtensionSharedGeneric<TB> {
  /// Creates a new shared wrapper around the provided extension instance.
  #[must_use]
  pub fn new(extension: PersistenceExtensionGeneric<TB>) -> Self {
    let mutex = <TB::MutexFamily as SyncMutexFamily>::create(extension);
    Self { inner: ArcShared::new(mutex) }
  }
}

impl<TB: RuntimeToolbox> Clone for PersistenceExtensionSharedGeneric<TB> {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}

impl<TB: RuntimeToolbox + 'static> SharedAccess<PersistenceExtensionGeneric<TB>>
  for PersistenceExtensionSharedGeneric<TB>
{
  fn with_read<R>(&self, f: impl FnOnce(&PersistenceExtensionGeneric<TB>) -> R) -> R {
    let guard = self.inner.lock();
    f(&*guard)
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut PersistenceExtensionGeneric<TB>) -> R) -> R {
    let mut guard = self.inner.lock();
    f(&mut *guard)
  }
}

impl<TB: RuntimeToolbox + 'static> Extension<TB> for PersistenceExtensionSharedGeneric<TB> {}
