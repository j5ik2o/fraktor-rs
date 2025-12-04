//! Shared wrapper for Dispatchers registry.

use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdToolbox, RuntimeToolbox, SyncMutexFamily, ToolboxMutex},
  sync::{ArcShared, SharedAccess},
};

use super::dispatchers::DispatchersGeneric;

/// Shared wrapper for [`DispatchersGeneric`].
///
/// This wrapper provides [`SharedAccess`] methods (`with_read`/`with_write`)
/// that internally lock the underlying registry, allowing safe
/// concurrent access from multiple owners.
pub struct DispatchersSharedGeneric<TB: RuntimeToolbox + 'static> {
  inner: ArcShared<ToolboxMutex<DispatchersGeneric<TB>, TB>>,
}

/// Type alias using the default toolbox.
pub type DispatchersShared = DispatchersSharedGeneric<NoStdToolbox>;

impl<TB: RuntimeToolbox + 'static> DispatchersSharedGeneric<TB> {
  /// Creates a new shared wrapper around the provided dispatcher registry.
  #[must_use]
  pub fn new(dispatchers: DispatchersGeneric<TB>) -> Self {
    Self { inner: ArcShared::new(<TB::MutexFamily as SyncMutexFamily>::create(dispatchers)) }
  }
}

impl<TB: RuntimeToolbox + 'static> Default for DispatchersSharedGeneric<TB> {
  fn default() -> Self {
    Self::new(DispatchersGeneric::default())
  }
}

impl<TB: RuntimeToolbox> Clone for DispatchersSharedGeneric<TB> {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}

impl<TB: RuntimeToolbox + 'static> SharedAccess<DispatchersGeneric<TB>> for DispatchersSharedGeneric<TB> {
  fn with_read<R>(&self, f: impl FnOnce(&DispatchersGeneric<TB>) -> R) -> R {
    self.inner.with_read(f)
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut DispatchersGeneric<TB>) -> R) -> R {
    self.inner.with_write(f)
  }
}
