//! Shared wrapper for endpoint manager.

use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdToolbox, RuntimeToolbox, SyncRwLockFamily, ToolboxRwLock},
  sync::{ArcShared, SharedAccess, sync_rwlock_like::SyncRwLockLike},
};

use super::EndpointManager;

/// Shared wrapper for [`EndpointManager`] enabling interior mutability.
///
/// This wrapper provides [`SharedAccess`] methods (`with_read`/`with_write`)
/// that internally lock the underlying [`EndpointManager`], allowing safe
/// concurrent access from multiple owners.
pub struct EndpointManagerSharedGeneric<TB: RuntimeToolbox + 'static> {
  inner: ArcShared<ToolboxRwLock<EndpointManager, TB>>,
}

impl<TB: RuntimeToolbox + 'static> Clone for EndpointManagerSharedGeneric<TB> {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}

impl<TB: RuntimeToolbox + 'static> Default for EndpointManagerSharedGeneric<TB> {
  fn default() -> Self {
    Self::new()
  }
}

impl<TB: RuntimeToolbox + 'static> EndpointManagerSharedGeneric<TB> {
  /// Creates a new shared endpoint manager instance.
  #[must_use]
  pub fn new() -> Self {
    Self { inner: ArcShared::new(<TB::RwLockFamily as SyncRwLockFamily>::create(EndpointManager::new())) }
  }
}

impl<TB: RuntimeToolbox + 'static> SharedAccess<EndpointManager> for EndpointManagerSharedGeneric<TB> {
  fn with_read<R>(&self, f: impl FnOnce(&EndpointManager) -> R) -> R {
    let guard = self.inner.read();
    f(&guard)
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut EndpointManager) -> R) -> R {
    let mut guard = self.inner.write();
    f(&mut guard)
  }
}

/// Type alias for [`EndpointManagerSharedGeneric`] using the default [`NoStdToolbox`].
pub type EndpointManagerShared = EndpointManagerSharedGeneric<NoStdToolbox>;
