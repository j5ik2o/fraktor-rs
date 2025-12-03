//! Shared wrapper for endpoint manager.

use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdToolbox, RuntimeToolbox, SyncMutexFamily, ToolboxMutex},
  sync::sync_mutex_like::SyncMutexLike,
};

use super::{AssociationState, EndpointManager, EndpointManagerCommand, EndpointManagerResult};

/// Shared wrapper for [`EndpointManager`] enabling interior mutability.
///
/// This wrapper provides `&self` methods that internally lock the underlying
/// [`EndpointManager`], allowing safe concurrent access from multiple owners.
pub struct EndpointManagerSharedGeneric<TB: RuntimeToolbox + 'static> {
  inner: ToolboxMutex<EndpointManager, TB>,
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
    Self { inner: <TB::MutexFamily as SyncMutexFamily>::create(EndpointManager::new()) }
  }

  /// Returns the current association state for the provided authority when available.
  #[must_use]
  pub fn state(&self, authority: &str) -> Option<AssociationState> {
    self.inner.lock().state(authority)
  }

  /// Handles a command and returns the produced effects.
  pub fn handle(&self, command: EndpointManagerCommand) -> EndpointManagerResult {
    self.inner.lock().handle(command)
  }
}

/// Type alias for [`EndpointManagerSharedGeneric`] using the default [`NoStdToolbox`].
pub type EndpointManagerShared = EndpointManagerSharedGeneric<NoStdToolbox>;
