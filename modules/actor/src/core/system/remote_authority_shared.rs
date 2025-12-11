//! Shared wrapper for RemoteAuthorityManager implementations.

use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdToolbox, RuntimeToolbox, SyncRwLockFamily, ToolboxRwLock},
  sync::{ArcShared, SharedAccess, sync_rwlock_like::SyncRwLockLike},
};

use super::remote_authority::RemoteAuthorityManagerGeneric;

/// Shared wrapper for [`RemoteAuthorityManagerGeneric`].
///
/// This wrapper provides [`SharedAccess`] methods (`with_read`/`with_write`)
/// that internally lock the underlying manager, allowing safe
/// concurrent access from multiple owners.
pub struct RemoteAuthorityManagerSharedGeneric<TB: RuntimeToolbox + 'static> {
  inner: ArcShared<ToolboxRwLock<RemoteAuthorityManagerGeneric<TB>, TB>>,
}

/// Type alias using the default toolbox.
pub type RemoteAuthorityManagerShared = RemoteAuthorityManagerSharedGeneric<NoStdToolbox>;

impl<TB: RuntimeToolbox + 'static> RemoteAuthorityManagerSharedGeneric<TB> {
  /// Creates a new shared wrapper around the provided manager.
  #[must_use]
  pub fn new(manager: RemoteAuthorityManagerGeneric<TB>) -> Self {
    Self { inner: ArcShared::new(<TB::RwLockFamily as SyncRwLockFamily>::create(manager)) }
  }
}

impl<TB: RuntimeToolbox + 'static> Default for RemoteAuthorityManagerSharedGeneric<TB> {
  fn default() -> Self {
    Self::new(RemoteAuthorityManagerGeneric::default())
  }
}

impl<TB: RuntimeToolbox> Clone for RemoteAuthorityManagerSharedGeneric<TB> {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}

impl<TB: RuntimeToolbox + 'static> SharedAccess<RemoteAuthorityManagerGeneric<TB>>
  for RemoteAuthorityManagerSharedGeneric<TB>
{
  fn with_read<R>(&self, f: impl FnOnce(&RemoteAuthorityManagerGeneric<TB>) -> R) -> R {
    let guard = self.inner.read();
    f(&guard)
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut RemoteAuthorityManagerGeneric<TB>) -> R) -> R {
    let mut guard = self.inner.write();
    f(&mut guard)
  }
}
