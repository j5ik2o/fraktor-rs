//! Shared wrapper for actor reference provider callers registry.

use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdToolbox, RuntimeToolbox, SyncMutexFamily, ToolboxMutex},
  sync::{ArcShared, SharedAccess, sync_mutex_like::SyncMutexLike},
};

use super::actor_ref_provider_callers::ActorRefProviderCallersGeneric;

/// Shared wrapper for [`ActorRefProviderCallersGeneric`].
///
/// This wrapper provides [`SharedAccess`] methods (`with_read`/`with_write`)
/// that internally lock the underlying registry, allowing safe
/// concurrent access from multiple owners.
pub(crate) struct ActorRefProviderCallersSharedGeneric<TB: RuntimeToolbox + 'static> {
  inner: ArcShared<ToolboxMutex<ActorRefProviderCallersGeneric<TB>, TB>>,
}

/// Type alias using the default toolbox.
#[allow(dead_code)]
pub(crate) type ActorRefProviderCallersShared = ActorRefProviderCallersSharedGeneric<NoStdToolbox>;

impl<TB: RuntimeToolbox + 'static> ActorRefProviderCallersSharedGeneric<TB> {
  /// Creates a new shared wrapper around the provided actor reference provider callers registry.
  #[must_use]
  pub(crate) fn new(callers: ActorRefProviderCallersGeneric<TB>) -> Self {
    Self { inner: ArcShared::new(<TB::MutexFamily as SyncMutexFamily>::create(callers)) }
  }
}

impl<TB: RuntimeToolbox + 'static> Default for ActorRefProviderCallersSharedGeneric<TB> {
  fn default() -> Self {
    Self::new(ActorRefProviderCallersGeneric::default())
  }
}

impl<TB: RuntimeToolbox> Clone for ActorRefProviderCallersSharedGeneric<TB> {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}

impl<TB: RuntimeToolbox + 'static> SharedAccess<ActorRefProviderCallersGeneric<TB>>
  for ActorRefProviderCallersSharedGeneric<TB>
{
  fn with_read<R>(&self, f: impl FnOnce(&ActorRefProviderCallersGeneric<TB>) -> R) -> R {
    let guard = self.inner.lock();
    f(&guard)
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut ActorRefProviderCallersGeneric<TB>) -> R) -> R {
    let mut guard = self.inner.lock();
    f(&mut guard)
  }
}
