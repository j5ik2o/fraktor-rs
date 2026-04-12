//! Shared wrapper for actor instance.

use alloc::boxed::Box;

use fraktor_utils_core_rs::core::sync::{SharedAccess, SharedLock};

use super::actor_lifecycle::Actor;

/// Shared wrapper for an actor instance.
///
/// This wrapper provides [`SharedAccess`] methods (`with_read`/`with_write`)
/// that internally lock the underlying actor, allowing safe
/// concurrent access from multiple owners.
pub(crate) struct ActorShared {
  inner: SharedLock<Box<dyn Actor + Send>>,
}
#[allow(dead_code)]
impl ActorShared {
  #[must_use]
  pub(crate) const fn from_shared_lock(inner: SharedLock<Box<dyn Actor + Send>>) -> Self {
    Self { inner }
  }
}

impl Clone for ActorShared {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
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
