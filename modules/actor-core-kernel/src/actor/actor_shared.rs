//! Shared wrapper for actor instance.

use alloc::boxed::Box;

use fraktor_utils_core_rs::sync::{DefaultMutex, SharedAccess, SharedLock};

use super::actor_lifecycle::Actor;

/// Shared wrapper for an actor instance.
///
/// This wrapper provides [`SharedAccess`] methods (`with_read`/`with_write`)
/// that serialize access to the underlying actor, allowing safe concurrent
/// access from multiple owners.
#[derive(Clone)]
pub struct ActorShared {
  inner: SharedLock<Box<dyn Actor + Send>>,
}

impl ActorShared {
  /// Creates a new shared wrapper around the provided actor.
  #[must_use]
  pub fn new(actor: Box<dyn Actor + Send>) -> Self {
    Self { inner: SharedLock::new_with_driver::<DefaultMutex<_>>(actor) }
  }
}

impl SharedAccess<Box<dyn Actor + Send>> for ActorShared {
  fn with_read<R>(&self, f: impl FnOnce(&Box<dyn Actor + Send>) -> R) -> R {
    self.inner.with_read(f)
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut Box<dyn Actor + Send>) -> R) -> R {
    self.inner.with_write(f)
  }
}
