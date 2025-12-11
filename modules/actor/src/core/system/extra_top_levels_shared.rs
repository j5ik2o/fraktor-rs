//! Shared wrapper for extra top-levels registry.

use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdToolbox, RuntimeToolbox, SyncMutexFamily, ToolboxMutex},
  sync::{ArcShared, SharedAccess, sync_mutex_like::SyncMutexLike},
};

use super::extra_top_levels::ExtraTopLevelsGeneric;

/// Shared wrapper for [`ExtraTopLevelsGeneric`].
///
/// This wrapper provides [`SharedAccess`] methods (`with_read`/`with_write`)
/// that internally lock the underlying registry, allowing safe
/// concurrent access from multiple owners.
pub(crate) struct ExtraTopLevelsSharedGeneric<TB: RuntimeToolbox + 'static> {
  inner: ArcShared<ToolboxMutex<ExtraTopLevelsGeneric<TB>, TB>>,
}

/// Type alias using the default toolbox.
#[allow(dead_code)]
pub(crate) type ExtraTopLevelsShared = ExtraTopLevelsSharedGeneric<NoStdToolbox>;

impl<TB: RuntimeToolbox + 'static> ExtraTopLevelsSharedGeneric<TB> {
  /// Creates a new shared wrapper around the provided extra top-levels registry.
  #[must_use]
  pub(crate) fn new(extra_top_levels: ExtraTopLevelsGeneric<TB>) -> Self {
    Self { inner: ArcShared::new(<TB::MutexFamily as SyncMutexFamily>::create(extra_top_levels)) }
  }
}

impl<TB: RuntimeToolbox + 'static> Default for ExtraTopLevelsSharedGeneric<TB> {
  fn default() -> Self {
    Self::new(ExtraTopLevelsGeneric::default())
  }
}

impl<TB: RuntimeToolbox> Clone for ExtraTopLevelsSharedGeneric<TB> {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}

impl<TB: RuntimeToolbox + 'static> SharedAccess<ExtraTopLevelsGeneric<TB>> for ExtraTopLevelsSharedGeneric<TB> {
  fn with_read<R>(&self, f: impl FnOnce(&ExtraTopLevelsGeneric<TB>) -> R) -> R {
    let guard = self.inner.lock();
    f(&guard)
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut ExtraTopLevelsGeneric<TB>) -> R) -> R {
    let mut guard = self.inner.lock();
    f(&mut guard)
  }
}
