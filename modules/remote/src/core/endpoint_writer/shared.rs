//! Shared wrapper for endpoint writer with interior mutability.

use fraktor_utils_rs::core::sync::{ArcShared, RuntimeMutex, SharedAccess};

use super::EndpointWriter;

/// Shared wrapper for an endpoint writer instance.
///
/// This wrapper provides [`SharedAccess`] methods (`with_read`/`with_write`)
/// that internally lock the underlying writer, allowing safe
/// concurrent access from multiple owners.
pub struct EndpointWriterShared {
  inner: ArcShared<RuntimeMutex<EndpointWriter>>,
}

impl EndpointWriterShared {
  /// Creates a new shared wrapper around the provided writer instance.
  #[must_use]
  pub fn new(writer: EndpointWriter) -> Self {
    let mutex = RuntimeMutex::new(writer);
    Self { inner: ArcShared::new(mutex) }
  }
}

impl Clone for EndpointWriterShared {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}

impl SharedAccess<EndpointWriter> for EndpointWriterShared {
  fn with_read<R>(&self, f: impl FnOnce(&EndpointWriter) -> R) -> R {
    let guard = self.inner.lock();
    f(&guard)
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut EndpointWriter) -> R) -> R {
    let mut guard = self.inner.lock();
    f(&mut guard)
  }
}
