//! Shared wrapper for serialization extension instance.

use fraktor_utils_rs::core::sync::{ArcShared, RuntimeMutex, SharedAccess};

use super::extension::SerializationExtension;
use crate::core::extension::Extension;

/// Shared wrapper for a serialization extension instance.
///
/// This wrapper provides [`SharedAccess`] methods (`with_read`/`with_write`)
/// that internally lock the underlying extension, allowing safe
/// concurrent access from multiple owners.
pub struct SerializationExtensionShared {
  inner: ArcShared<RuntimeMutex<SerializationExtension>>,
}

impl SerializationExtensionShared {
  /// Creates a new shared wrapper around the provided extension instance.
  #[must_use]
  pub fn new(extension: SerializationExtension) -> Self {
    let mutex = RuntimeMutex::new(extension);
    Self { inner: ArcShared::new(mutex) }
  }
}

impl Clone for SerializationExtensionShared {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}

impl SharedAccess<SerializationExtension> for SerializationExtensionShared {
  fn with_read<R>(&self, f: impl FnOnce(&SerializationExtension) -> R) -> R {
    let guard = self.inner.lock();
    f(&guard)
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut SerializationExtension) -> R) -> R {
    let mut guard = self.inner.lock();
    f(&mut guard)
  }
}

impl Extension for SerializationExtensionShared {}
