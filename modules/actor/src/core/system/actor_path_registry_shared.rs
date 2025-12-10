//! Shared wrapper for actor path registry.

use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdToolbox, RuntimeToolbox, SyncMutexFamily, ToolboxMutex},
  sync::{ArcShared, SharedAccess},
};

use super::actor_path_registry::ActorPathRegistry;

/// Shared wrapper for [`ActorPathRegistry`].
///
/// This wrapper provides [`SharedAccess`] methods (`with_read`/`with_write`)
/// that internally lock the underlying registry, allowing safe
/// concurrent access from multiple owners.
pub struct ActorPathRegistrySharedGeneric<TB: RuntimeToolbox + 'static> {
  inner: ArcShared<ToolboxMutex<ActorPathRegistry, TB>>,
}

/// Type alias using the default toolbox.
pub type ActorPathRegistryShared = ActorPathRegistrySharedGeneric<NoStdToolbox>;

impl<TB: RuntimeToolbox + 'static> ActorPathRegistrySharedGeneric<TB> {
  /// Creates a new shared wrapper around the provided actor path registry.
  #[must_use]
  pub fn new(registry: ActorPathRegistry) -> Self {
    Self { inner: ArcShared::new(<TB::MutexFamily as SyncMutexFamily>::create(registry)) }
  }
}

impl<TB: RuntimeToolbox + 'static> Default for ActorPathRegistrySharedGeneric<TB> {
  fn default() -> Self {
    Self::new(ActorPathRegistry::default())
  }
}

impl<TB: RuntimeToolbox> Clone for ActorPathRegistrySharedGeneric<TB> {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}

impl<TB: RuntimeToolbox + 'static> SharedAccess<ActorPathRegistry> for ActorPathRegistrySharedGeneric<TB> {
  fn with_read<R>(&self, f: impl FnOnce(&ActorPathRegistry) -> R) -> R {
    self.inner.with_read(f)
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut ActorPathRegistry) -> R) -> R {
    self.inner.with_write(f)
  }
}
