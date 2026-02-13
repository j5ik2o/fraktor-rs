//! Shared wrapper for MessageInvokerMiddleware implementations.

use alloc::boxed::Box;

use fraktor_utils_rs::core::{
  runtime_toolbox::{RuntimeToolbox, ToolboxRwLock, sync_rwlock_family::SyncRwLockFamily},
  sync::{ArcShared, SharedAccess, sync_rwlock_like::SyncRwLockLike},
};

use super::middleware::MessageInvokerMiddleware;

/// Shared wrapper for [`MessageInvokerMiddleware`] implementations.
///
/// This wrapper provides [`SharedAccess`] methods (`with_read`/`with_write`)
/// that internally lock the underlying middleware, allowing safe
/// concurrent access from multiple owners.
pub(crate) struct MiddlewareShared<TB: RuntimeToolbox + 'static> {
  inner: ArcShared<ToolboxRwLock<Box<dyn MessageInvokerMiddleware<TB>>, TB>>,
}

impl<TB: RuntimeToolbox + 'static> MiddlewareShared<TB> {
  /// Creates a new shared wrapper around the provided middleware.
  #[must_use]
  #[allow(dead_code)] // Used in tests
  pub(crate) fn new(middleware: Box<dyn MessageInvokerMiddleware<TB>>) -> Self {
    Self { inner: ArcShared::new(<TB::RwLockFamily as SyncRwLockFamily>::create(middleware)) }
  }
}

impl<TB: RuntimeToolbox + 'static> Clone for MiddlewareShared<TB> {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}

impl<TB: RuntimeToolbox + 'static> SharedAccess<Box<dyn MessageInvokerMiddleware<TB>>> for MiddlewareShared<TB> {
  fn with_read<R>(&self, f: impl FnOnce(&Box<dyn MessageInvokerMiddleware<TB>>) -> R) -> R {
    let guard = self.inner.read();
    f(&guard)
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut Box<dyn MessageInvokerMiddleware<TB>>) -> R) -> R {
    let mut guard = self.inner.write();
    f(&mut guard)
  }
}
