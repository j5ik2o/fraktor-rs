//! Shared wrapper for actor instance.

use alloc::boxed::Box;

use fraktor_utils_core_rs::core::sync::{SharedAccess, SharedLock, SpinSyncMutex};

use super::actor_lifecycle::Actor;

/// Shared wrapper for an actor instance.
///
/// This wrapper provides [`SharedAccess`] methods (`with_read`/`with_write`)
/// that internally lock the underlying actor, allowing safe
/// concurrent access from multiple owners.
pub(crate) struct ActorShared {
  inner: SharedLock<Box<dyn Actor + Send + Sync>>,
}
#[allow(dead_code)]
impl ActorShared {
  /// Creates a new shared wrapper around the provided actor instance.
  #[must_use]
  pub(crate) fn new(actor: Box<dyn Actor + Send + Sync>) -> Self {
    Self { inner: SharedLock::new_with_driver::<SpinSyncMutex<_>>(actor) }
  }
}

impl Clone for ActorShared {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}

impl SharedAccess<Box<dyn Actor + Send + Sync>> for ActorShared {
  fn with_read<R>(&self, f: impl FnOnce(&Box<dyn Actor + Send + Sync>) -> R) -> R {
    self.inner.with_read(f)
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut Box<dyn Actor + Send + Sync>) -> R) -> R {
    self.inner.with_write(f)
  }
}
