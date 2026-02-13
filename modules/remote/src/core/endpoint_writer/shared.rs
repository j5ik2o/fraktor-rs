//! Shared wrapper for endpoint writer with interior mutability.

use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdToolbox, RuntimeToolbox, ToolboxMutex, sync_mutex_family::SyncMutexFamily},
  sync::{ArcShared, SharedAccess, sync_mutex_like::SyncMutexLike},
};

use super::EndpointWriterGeneric;

/// Shared wrapper for an endpoint writer instance.
///
/// This wrapper provides [`SharedAccess`] methods (`with_read`/`with_write`)
/// that internally lock the underlying writer, allowing safe
/// concurrent access from multiple owners.
pub struct EndpointWriterSharedGeneric<TB: RuntimeToolbox + 'static> {
  inner: ArcShared<ToolboxMutex<EndpointWriterGeneric<TB>, TB>>,
}

/// Type alias using the default toolbox.
pub type EndpointWriterShared = EndpointWriterSharedGeneric<NoStdToolbox>;

impl<TB: RuntimeToolbox + 'static> EndpointWriterSharedGeneric<TB> {
  /// Creates a new shared wrapper around the provided writer instance.
  #[must_use]
  pub fn new(writer: EndpointWriterGeneric<TB>) -> Self {
    let mutex = <TB::MutexFamily as SyncMutexFamily>::create(writer);
    Self { inner: ArcShared::new(mutex) }
  }
}

impl<TB: RuntimeToolbox> Clone for EndpointWriterSharedGeneric<TB> {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}

impl<TB: RuntimeToolbox + 'static> SharedAccess<EndpointWriterGeneric<TB>> for EndpointWriterSharedGeneric<TB> {
  fn with_read<R>(&self, f: impl FnOnce(&EndpointWriterGeneric<TB>) -> R) -> R {
    let guard = self.inner.lock();
    f(&*guard)
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut EndpointWriterGeneric<TB>) -> R) -> R {
    let mut guard = self.inner.lock();
    f(&mut *guard)
  }
}
