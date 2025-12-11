//! Shared wrapper for actor cell registry.

use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdToolbox, RuntimeToolbox, SyncMutexFamily, ToolboxMutex},
  sync::{ArcShared, SharedAccess, sync_mutex_like::SyncMutexLike},
};

use super::cells::CellsGeneric;

/// Shared wrapper for [`CellsGeneric`].
///
/// This wrapper provides [`SharedAccess`] methods (`with_read`/`with_write`)
/// that internally lock the underlying registry, allowing safe
/// concurrent access from multiple owners.
pub(crate) struct CellsSharedGeneric<TB: RuntimeToolbox + 'static> {
  inner: ArcShared<ToolboxMutex<CellsGeneric<TB>, TB>>,
}

/// Type alias using the default toolbox.
#[allow(dead_code)]
pub(crate) type CellsShared = CellsSharedGeneric<NoStdToolbox>;

impl<TB: RuntimeToolbox + 'static> CellsSharedGeneric<TB> {
  /// Creates a new shared wrapper around the provided registry.
  #[must_use]
  pub(crate) fn new(cells: CellsGeneric<TB>) -> Self {
    Self { inner: ArcShared::new(<TB::MutexFamily as SyncMutexFamily>::create(cells)) }
  }
}

impl<TB: RuntimeToolbox + 'static> Default for CellsSharedGeneric<TB> {
  fn default() -> Self {
    Self::new(CellsGeneric::default())
  }
}

impl<TB: RuntimeToolbox> Clone for CellsSharedGeneric<TB> {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}

impl<TB: RuntimeToolbox + 'static> SharedAccess<CellsGeneric<TB>> for CellsSharedGeneric<TB> {
  fn with_read<R>(&self, f: impl FnOnce(&CellsGeneric<TB>) -> R) -> R {
    let guard = self.inner.lock();
    f(&guard)
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut CellsGeneric<TB>) -> R) -> R {
    let mut guard = self.inner.lock();
    f(&mut guard)
  }
}
