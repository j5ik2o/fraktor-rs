//! Shared wrapper for actor instance.

use alloc::boxed::Box;

use fraktor_utils_rs::core::{
  runtime_toolbox::RuntimeMutex,
  sync::{ArcShared, SharedAccess},
};

use super::actor_lifecycle::Actor;

/// Shared wrapper for an actor instance.
///
/// This wrapper provides [`SharedAccess`] methods (`with_read`/`with_write`)
/// that internally lock the underlying actor, allowing safe
/// concurrent access from multiple owners.
pub(crate) struct ActorShared {
  inner: ArcShared<RuntimeMutex<Box<dyn Actor + Send + Sync>>>,
}
#[allow(dead_code)]
impl ActorShared {
  /// Creates a new shared wrapper around the provided actor instance.
  #[must_use]
  pub(crate) fn new(actor: Box<dyn Actor + Send + Sync>) -> Self {
    let mutex = RuntimeMutex::new(actor);
    Self { inner: ArcShared::new(mutex) }
  }
}

impl Clone for ActorShared {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}

impl SharedAccess<Box<dyn Actor + Send + Sync>> for ActorShared {
  fn with_read<R>(&self, f: impl FnOnce(&Box<dyn Actor + Send + Sync>) -> R) -> R {
    let guard = self.inner.lock();
    f(&guard)
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut Box<dyn Actor + Send + Sync>) -> R) -> R {
    let mut guard = self.inner.lock();
    f(&mut guard)
  }
}
