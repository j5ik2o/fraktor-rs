//! Shared wrapper for actor factory.

use alloc::boxed::Box;

use fraktor_utils_core_rs::core::sync::{ArcShared, RuntimeMutex, SharedAccess};

use super::factory::ActorFactory;

/// Shared wrapper for [`ActorFactory`].
///
/// This wrapper provides [`SharedAccess`] methods (`with_read`/`with_write`)
/// that internally lock the underlying factory, allowing safe
/// concurrent access from multiple owners.
pub struct ActorFactoryShared {
  inner: ArcShared<RuntimeMutex<Box<dyn ActorFactory>>>,
}

impl ActorFactoryShared {
  /// Creates a new shared wrapper around the provided actor factory.
  #[must_use]
  pub fn new(factory: Box<dyn ActorFactory>) -> Self {
    Self { inner: ArcShared::new(RuntimeMutex::new(factory)) }
  }
}

impl Clone for ActorFactoryShared {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}

impl SharedAccess<Box<dyn ActorFactory>> for ActorFactoryShared {
  fn with_read<R>(&self, f: impl FnOnce(&Box<dyn ActorFactory>) -> R) -> R {
    let guard = self.inner.lock();
    f(&guard)
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut Box<dyn ActorFactory>) -> R) -> R {
    let mut guard = self.inner.lock();
    f(&mut guard)
  }
}
