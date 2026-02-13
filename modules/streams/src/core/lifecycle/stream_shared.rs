use fraktor_utils_rs::core::{
  runtime_toolbox::{RuntimeToolbox, ToolboxMutex, sync_mutex_family::SyncMutexFamily},
  sync::{ArcShared, SharedAccess, sync_mutex_like::SyncMutexLike},
};

use super::stream::Stream;

/// Shared wrapper for [`Stream`].
pub(crate) struct StreamSharedGeneric<TB: RuntimeToolbox + 'static> {
  inner: ArcShared<ToolboxMutex<Stream, TB>>,
}

impl<TB: RuntimeToolbox + 'static> Clone for StreamSharedGeneric<TB> {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}

impl<TB: RuntimeToolbox + 'static> StreamSharedGeneric<TB> {
  pub(crate) fn new(stream: Stream) -> Self {
    let inner = ArcShared::new(<TB::MutexFamily as SyncMutexFamily>::create(stream));
    Self { inner }
  }
}

impl<TB: RuntimeToolbox + 'static> SharedAccess<Stream> for StreamSharedGeneric<TB> {
  fn with_read<R>(&self, f: impl FnOnce(&Stream) -> R) -> R {
    let guard = self.inner.lock();
    f(&guard)
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut Stream) -> R) -> R {
    let mut guard = self.inner.lock();
    f(&mut guard)
  }
}
