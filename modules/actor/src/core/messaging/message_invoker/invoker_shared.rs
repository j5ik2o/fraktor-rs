//! Shared wrapper for MessageInvoker implementations.

use alloc::boxed::Box;

use fraktor_utils_rs::core::{
  runtime_toolbox::{RuntimeToolbox, SyncMutexFamily, ToolboxMutex},
  sync::{ArcShared, SharedAccess, sync_mutex_like::SyncMutexLike},
};

use super::invoker_trait::MessageInvoker;

/// Shared wrapper for [`MessageInvoker`] implementations.
///
/// This wrapper provides [`SharedAccess`] methods (`with_read`/`with_write`)
/// that internally lock the underlying invoker, allowing safe
/// concurrent access from multiple owners.
pub struct MessageInvokerShared<TB: RuntimeToolbox + 'static> {
  inner: ArcShared<ToolboxMutex<Box<dyn MessageInvoker<TB>>, TB>>,
}

impl<TB: RuntimeToolbox + 'static> MessageInvokerShared<TB> {
  /// Creates a new shared wrapper around the provided invoker.
  #[must_use]
  pub fn new(invoker: Box<dyn MessageInvoker<TB>>) -> Self {
    Self { inner: ArcShared::new(<TB::MutexFamily as SyncMutexFamily>::create(invoker)) }
  }
}

impl<TB: RuntimeToolbox + 'static> Clone for MessageInvokerShared<TB> {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}

impl<TB: RuntimeToolbox + 'static> SharedAccess<Box<dyn MessageInvoker<TB>>> for MessageInvokerShared<TB> {
  fn with_read<R>(&self, f: impl FnOnce(&Box<dyn MessageInvoker<TB>>) -> R) -> R {
    let guard = self.inner.lock();
    f(&guard)
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut Box<dyn MessageInvoker<TB>>) -> R) -> R {
    let mut guard = self.inner.lock();
    f(&mut guard)
  }
}
