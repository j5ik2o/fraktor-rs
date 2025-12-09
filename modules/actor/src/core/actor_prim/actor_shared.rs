//! Shared wrapper for actor instance.

use alloc::boxed::Box;

use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdToolbox, RuntimeToolbox, SyncMutexFamily, ToolboxMutex},
  sync::{SharedAccess, sync_mutex_like::SyncMutexLike},
};

use super::actor::Actor;

/// Shared wrapper for an actor instance.
///
/// This wrapper provides [`SharedAccess`] methods (`with_read`/`with_write`)
/// that internally lock the underlying actor, allowing safe
/// concurrent access from multiple owners.
pub(crate) struct ActorSharedGeneric<TB: RuntimeToolbox + 'static> {
  inner: ToolboxMutex<Box<dyn Actor<TB> + Send + Sync>, TB>,
}

/// Type alias using the default toolbox.
#[allow(dead_code)]
pub(crate) type ActorShared = ActorSharedGeneric<NoStdToolbox>;

impl<TB: RuntimeToolbox + 'static> ActorSharedGeneric<TB> {
  /// Creates a new shared wrapper around the provided actor instance.
  #[must_use]
  pub(crate) fn new(actor: Box<dyn Actor<TB> + Send + Sync>) -> Self {
    Self { inner: <TB::MutexFamily as SyncMutexFamily>::create(actor) }
  }
}

impl<TB: RuntimeToolbox + 'static> SharedAccess<Box<dyn Actor<TB> + Send + Sync>> for ActorSharedGeneric<TB> {
  fn with_read<R>(&self, f: impl FnOnce(&Box<dyn Actor<TB> + Send + Sync>) -> R) -> R {
    let guard = self.inner.lock();
    f(&*guard)
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut Box<dyn Actor<TB> + Send + Sync>) -> R) -> R {
    let mut guard = self.inner.lock();
    f(&mut *guard)
  }
}
