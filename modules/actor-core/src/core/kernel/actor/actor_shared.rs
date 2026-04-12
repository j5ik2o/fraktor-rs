//! Shared wrapper for actor instance.

use alloc::boxed::Box;

use fraktor_utils_core_rs::core::sync::{SharedAccess, SharedLock};

use super::actor_lifecycle::Actor;

/// Shared wrapper for an actor instance.
///
/// This wrapper provides [`SharedAccess`] methods (`with_read`/`with_write`)
/// that internally lock the underlying actor, allowing safe
/// concurrent access from multiple owners.
#[derive(Clone)]
pub struct ActorShared {
  inner: SharedLock<Box<dyn Actor + Send>>,
}

impl ActorShared {
  /// Creates an `ActorShared` wrapper from an existing shared lock.
  #[must_use]
  pub const fn from_shared_lock(inner: SharedLock<Box<dyn Actor + Send>>) -> Self {
    Self { inner }
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
