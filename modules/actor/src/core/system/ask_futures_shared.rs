//! Shared wrapper for ask futures registry.

use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdToolbox, RuntimeToolbox, SyncMutexFamily, ToolboxMutex},
  sync::{ArcShared, SharedAccess, sync_mutex_like::SyncMutexLike},
};

use super::ask_futures::AskFuturesGeneric;

/// Shared wrapper for [`AskFuturesGeneric`].
///
/// This wrapper provides [`SharedAccess`] methods (`with_read`/`with_write`)
/// that internally lock the underlying registry, allowing safe
/// concurrent access from multiple owners.
pub(crate) struct AskFuturesSharedGeneric<TB: RuntimeToolbox + 'static> {
  inner: ArcShared<ToolboxMutex<AskFuturesGeneric<TB>, TB>>,
}

/// Type alias using the default toolbox.
#[allow(dead_code)]
pub(crate) type AskFuturesShared = AskFuturesSharedGeneric<NoStdToolbox>;

impl<TB: RuntimeToolbox + 'static> AskFuturesSharedGeneric<TB> {
  /// Creates a new shared wrapper around the provided ask futures registry.
  #[must_use]
  pub(crate) fn new(ask_futures: AskFuturesGeneric<TB>) -> Self {
    Self { inner: ArcShared::new(<TB::MutexFamily as SyncMutexFamily>::create(ask_futures)) }
  }
}

impl<TB: RuntimeToolbox + 'static> Default for AskFuturesSharedGeneric<TB> {
  fn default() -> Self {
    Self::new(AskFuturesGeneric::default())
  }
}

impl<TB: RuntimeToolbox> Clone for AskFuturesSharedGeneric<TB> {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}

impl<TB: RuntimeToolbox + 'static> SharedAccess<AskFuturesGeneric<TB>> for AskFuturesSharedGeneric<TB> {
  fn with_read<R>(&self, f: impl FnOnce(&AskFuturesGeneric<TB>) -> R) -> R {
    let guard = self.inner.lock();
    f(&guard)
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut AskFuturesGeneric<TB>) -> R) -> R {
    let mut guard = self.inner.lock();
    f(&mut guard)
  }
}
