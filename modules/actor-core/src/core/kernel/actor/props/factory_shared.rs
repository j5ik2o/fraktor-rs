//! Shared wrapper for actor factory.

use alloc::boxed::Box;

use fraktor_utils_core_rs::core::sync::{DefaultMutex, SharedAccess, SharedLock};

use super::factory::ActorFactory;

/// Shared wrapper for [`ActorFactory`].
///
/// This wrapper provides [`SharedAccess`] methods (`with_read`/`with_write`)
/// that internally lock the underlying factory, allowing safe
/// concurrent access from multiple owners.
pub struct ActorFactoryShared {
  inner: SharedLock<Box<dyn ActorFactory>>,
}

impl ActorFactoryShared {
  /// Creates a new shared wrapper around the provided actor factory.
  #[must_use]
  pub fn new(factory: Box<dyn ActorFactory>) -> Self {
    Self { inner: SharedLock::new_with_driver::<DefaultMutex<_>>(factory) }
  }
}

impl Clone for ActorFactoryShared {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}

impl SharedAccess<Box<dyn ActorFactory>> for ActorFactoryShared {
  fn with_read<R>(&self, f: impl FnOnce(&Box<dyn ActorFactory>) -> R) -> R {
    self.inner.with_read(f)
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut Box<dyn ActorFactory>) -> R) -> R {
    self.inner.with_write(f)
  }
}
