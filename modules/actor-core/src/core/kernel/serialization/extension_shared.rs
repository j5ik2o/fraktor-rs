//! Shared wrapper for serialization extension instance.

use fraktor_utils_core_rs::core::sync::{SharedAccess, SharedLock, SpinSyncMutex};

use super::extension::SerializationExtension;
use crate::core::kernel::actor::extension::Extension;

/// Shared wrapper for a serialization extension instance.
///
/// This wrapper provides [`SharedAccess`] methods (`with_read`/`with_write`)
/// that internally lock the underlying extension, allowing safe
/// concurrent access from multiple owners.
pub struct SerializationExtensionShared {
  inner: SharedLock<SerializationExtension>,
}

impl SerializationExtensionShared {
  /// Creates a new shared wrapper around the provided extension instance.
  #[must_use]
  pub fn new(extension: SerializationExtension) -> Self {
    Self { inner: SharedLock::new_with_driver::<SpinSyncMutex<_>>(extension) }
  }
}

impl Clone for SerializationExtensionShared {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}

impl SharedAccess<SerializationExtension> for SerializationExtensionShared {
  fn with_read<R>(&self, f: impl FnOnce(&SerializationExtension) -> R) -> R {
    self.inner.with_read(f)
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut SerializationExtension) -> R) -> R {
    self.inner.with_write(f)
  }
}

impl Extension for SerializationExtensionShared {}
