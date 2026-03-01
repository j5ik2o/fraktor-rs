use core::marker::PhantomData;

use fraktor_utils_rs::core::{
  runtime_toolbox::{RuntimeMutex, RuntimeToolbox},
  sync::{ArcShared, SharedAccess, sync_mutex_like::SyncMutexLike},
};

use super::stream::Stream;

/// Shared wrapper for [`Stream`].
pub(crate) struct StreamSharedGeneric<TB: RuntimeToolbox + 'static> {
  inner:   ArcShared<RuntimeMutex<Stream>>,
  _marker: PhantomData<TB>,
}

impl<TB: RuntimeToolbox + 'static> Clone for StreamSharedGeneric<TB> {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone(), _marker: PhantomData }
  }
}

impl<TB: RuntimeToolbox + 'static> StreamSharedGeneric<TB> {
  pub(crate) fn new(stream: Stream) -> Self {
    let inner = ArcShared::new(RuntimeMutex::new(stream));
    Self { inner, _marker: PhantomData }
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
