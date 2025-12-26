//! Shared wrapper for RemoteAuthorityRegistry implementations.

use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdToolbox, RuntimeToolbox, SyncRwLockFamily, ToolboxRwLock},
  sync::{ArcShared, SharedAccess, sync_rwlock_like::SyncRwLockLike},
};

use super::remote_authority_registry::RemoteAuthorityRegistryGeneric;

/// Shared wrapper for [`RemoteAuthorityRegistryGeneric`].
///
/// This wrapper provides [`SharedAccess`] methods (`with_read`/`with_write`)
/// that internally lock the underlying registry, allowing safe
/// concurrent access from multiple owners.
pub struct RemoteAuthorityRegistrySharedGeneric<TB: RuntimeToolbox + 'static> {
  inner: ArcShared<ToolboxRwLock<RemoteAuthorityRegistryGeneric<TB>, TB>>,
}

/// Type alias using the default toolbox.
pub type RemoteAuthorityRegistryShared = RemoteAuthorityRegistrySharedGeneric<NoStdToolbox>;

impl<TB: RuntimeToolbox + 'static> RemoteAuthorityRegistrySharedGeneric<TB> {
  /// Creates a new shared wrapper around the provided registry.
  #[must_use]
  pub fn new(registry: RemoteAuthorityRegistryGeneric<TB>) -> Self {
    Self { inner: ArcShared::new(<TB::RwLockFamily as SyncRwLockFamily>::create(registry)) }
  }
}

impl<TB: RuntimeToolbox + 'static> Default for RemoteAuthorityRegistrySharedGeneric<TB> {
  fn default() -> Self {
    Self::new(RemoteAuthorityRegistryGeneric::default())
  }
}

impl<TB: RuntimeToolbox> Clone for RemoteAuthorityRegistrySharedGeneric<TB> {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}

impl<TB: RuntimeToolbox + 'static> SharedAccess<RemoteAuthorityRegistryGeneric<TB>>
  for RemoteAuthorityRegistrySharedGeneric<TB>
{
  fn with_read<R>(&self, f: impl FnOnce(&RemoteAuthorityRegistryGeneric<TB>) -> R) -> R {
    let guard = self.inner.read();
    f(&guard)
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut RemoteAuthorityRegistryGeneric<TB>) -> R) -> R {
    let mut guard = self.inner.write();
    f(&mut guard)
  }
}
