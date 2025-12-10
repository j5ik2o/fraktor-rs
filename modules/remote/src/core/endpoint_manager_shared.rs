//! Shared wrapper for endpoint manager.

use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdToolbox, RuntimeToolbox, SyncMutexFamily, ToolboxMutex},
  sync::{ArcShared, SharedAccess},
};

use super::EndpointManager;

/// Shared wrapper for [`EndpointManager`] enabling interior mutability.
///
/// This wrapper provides [`SharedAccess`] methods (`with_read`/`with_write`)
/// that internally lock the underlying [`EndpointManager`], allowing safe
/// concurrent access from multiple owners.
pub struct EndpointManagerSharedGeneric<TB: RuntimeToolbox + 'static> {
  inner: ArcShared<ToolboxMutex<EndpointManager, TB>>,
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
    Self { inner: ArcShared::new(<TB::MutexFamily as SyncMutexFamily>::create(EndpointManager::new())) }
  }
}

impl<TB: RuntimeToolbox + 'static> SharedAccess<EndpointManager> for EndpointManagerSharedGeneric<TB> {
  fn with_read<R>(&self, f: impl FnOnce(&EndpointManager) -> R) -> R {
    self.inner.with_read(f)
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut EndpointManager) -> R) -> R {
    self.inner.with_write(f)
  }
}

/// Type alias for [`EndpointManagerSharedGeneric`] using the default [`NoStdToolbox`].
pub type EndpointManagerShared = EndpointManagerSharedGeneric<NoStdToolbox>;
