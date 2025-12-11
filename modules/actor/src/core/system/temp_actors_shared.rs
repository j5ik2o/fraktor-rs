//! Shared wrapper for temporary actors registry.

use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdToolbox, RuntimeToolbox, SyncMutexFamily, ToolboxMutex},
  sync::{ArcShared, SharedAccess, sync_mutex_like::SyncMutexLike},
};

use super::temp_actors::TempActorsGeneric;

/// Shared wrapper for [`TempActorsGeneric`].
///
/// This wrapper provides [`SharedAccess`] methods (`with_read`/`with_write`)
/// that internally lock the underlying registry, allowing safe
/// concurrent access from multiple owners.
pub(crate) struct TempActorsSharedGeneric<TB: RuntimeToolbox + 'static> {
  inner: ArcShared<ToolboxMutex<TempActorsGeneric<TB>, TB>>,
}

/// Type alias using the default toolbox.
#[allow(dead_code)]
pub(crate) type TempActorsShared = TempActorsSharedGeneric<NoStdToolbox>;

impl<TB: RuntimeToolbox + 'static> TempActorsSharedGeneric<TB> {
  /// Creates a new shared wrapper around the provided temporary actors registry.
  #[must_use]
  pub(crate) fn new(temp_actors: TempActorsGeneric<TB>) -> Self {
    Self { inner: ArcShared::new(<TB::MutexFamily as SyncMutexFamily>::create(temp_actors)) }
  }
}

impl<TB: RuntimeToolbox + 'static> Default for TempActorsSharedGeneric<TB> {
  fn default() -> Self {
    Self::new(TempActorsGeneric::default())
  }
}

impl<TB: RuntimeToolbox> Clone for TempActorsSharedGeneric<TB> {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}

impl<TB: RuntimeToolbox + 'static> SharedAccess<TempActorsGeneric<TB>> for TempActorsSharedGeneric<TB> {
  fn with_read<R>(&self, f: impl FnOnce(&TempActorsGeneric<TB>) -> R) -> R {
    let guard = self.inner.lock();
    f(&guard)
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut TempActorsGeneric<TB>) -> R) -> R {
    let mut guard = self.inner.lock();
    f(&mut guard)
  }
}
