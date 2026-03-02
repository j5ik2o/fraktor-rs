use fraktor_utils_rs::core::sync::{ArcShared, RuntimeMutex, SharedAccess};

use super::stream::Stream;

/// Shared wrapper for [`Stream`].
pub(crate) struct StreamShared {
  inner: ArcShared<RuntimeMutex<Stream>>,
}

impl Clone for StreamShared {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}

impl StreamShared {
  pub(crate) fn new(stream: Stream) -> Self {
    let inner = ArcShared::new(RuntimeMutex::new(stream));
    Self { inner }
  }
}

impl SharedAccess<Stream> for StreamShared {
  fn with_read<R>(&self, f: impl FnOnce(&Stream) -> R) -> R {
    let guard = self.inner.lock();
    f(&guard)
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut Stream) -> R) -> R {
    let mut guard = self.inner.lock();
    f(&mut guard)
  }
}
