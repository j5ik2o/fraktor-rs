//! Shared wrapper for persistence extension instance.

use fraktor_actor_core_kernel_rs::actor::extension::Extension;
use fraktor_utils_core_rs::core::sync::{DefaultMutex, SharedAccess, SharedLock};

use crate::core::persistence_extension::PersistenceExtension;

/// Shared wrapper for a persistence extension instance.
pub struct PersistenceExtensionShared {
  inner: SharedLock<PersistenceExtension>,
}

impl PersistenceExtensionShared {
  /// Creates a new shared wrapper around the provided extension instance.
  #[must_use]
  pub fn new(extension: PersistenceExtension) -> Self {
    Self { inner: SharedLock::new_with_driver::<DefaultMutex<_>>(extension) }
  }
}

impl Clone for PersistenceExtensionShared {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}

impl SharedAccess<PersistenceExtension> for PersistenceExtensionShared {
  fn with_read<R>(&self, f: impl FnOnce(&PersistenceExtension) -> R) -> R {
    self.inner.with_read(f)
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut PersistenceExtension) -> R) -> R {
    self.inner.with_write(f)
  }
}

impl Extension for PersistenceExtensionShared {}
