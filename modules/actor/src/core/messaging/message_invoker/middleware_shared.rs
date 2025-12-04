//! Shared wrapper for MessageInvokerMiddleware implementations.

use alloc::boxed::Box;

use fraktor_utils_rs::core::{
  runtime_toolbox::{RuntimeToolbox, SyncMutexFamily, ToolboxMutex},
  sync::{ArcShared, SharedAccess, sync_mutex_like::SyncMutexLike},
};

use super::middleware::MessageInvokerMiddleware;

/// Shared wrapper for [`MessageInvokerMiddleware`] implementations.
///
/// This wrapper provides [`SharedAccess`] methods (`with_read`/`with_write`)
/// that internally lock the underlying middleware, allowing safe
/// concurrent access from multiple owners.
pub(crate) struct MiddlewareShared<TB: RuntimeToolbox + 'static> {
  inner: ArcShared<ToolboxMutex<Box<dyn MessageInvokerMiddleware<TB>>, TB>>,
}

impl<TB: RuntimeToolbox + 'static> MiddlewareShared<TB> {
  /// Creates a new shared wrapper around the provided middleware.
  #[must_use]
  #[allow(dead_code)] // Used in tests
  pub(crate) fn new(middleware: Box<dyn MessageInvokerMiddleware<TB>>) -> Self {
    Self { inner: ArcShared::new(<TB::MutexFamily as SyncMutexFamily>::create(middleware)) }
  }
}

impl<TB: RuntimeToolbox + 'static> Clone for MiddlewareShared<TB> {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}

impl<TB: RuntimeToolbox + 'static> SharedAccess<Box<dyn MessageInvokerMiddleware<TB>>> for MiddlewareShared<TB> {
  fn with_read<R>(&self, f: impl FnOnce(&Box<dyn MessageInvokerMiddleware<TB>>) -> R) -> R {
    f(&self.inner.lock())
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut Box<dyn MessageInvokerMiddleware<TB>>) -> R) -> R {
    f(&mut self.inner.lock())
  }
}
