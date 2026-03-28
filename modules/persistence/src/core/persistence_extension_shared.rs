//! Shared wrapper for persistence extension instance.

use fraktor_actor_rs::core::kernel::extension::Extension;
use fraktor_utils_rs::core::sync::{ArcShared, RuntimeMutex, SharedAccess};

use crate::core::persistence_extension::PersistenceExtension;

/// Shared wrapper for a persistence extension instance.
pub struct PersistenceExtensionShared {
  inner: ArcShared<RuntimeMutex<PersistenceExtension>>,
}

impl PersistenceExtensionShared {
  /// Creates a new shared wrapper around the provided extension instance.
  #[must_use]
  pub fn new(extension: PersistenceExtension) -> Self {
    let mutex = RuntimeMutex::new(extension);
    Self { inner: ArcShared::new(mutex) }
  }
}

impl Clone for PersistenceExtensionShared {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}

impl SharedAccess<PersistenceExtension> for PersistenceExtensionShared {
  fn with_read<R>(&self, f: impl FnOnce(&PersistenceExtension) -> R) -> R {
    let guard = self.inner.lock();
    f(&guard)
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut PersistenceExtension) -> R) -> R {
    let mut guard = self.inner.lock();
    f(&mut guard)
  }
}

impl Extension for PersistenceExtensionShared {}
