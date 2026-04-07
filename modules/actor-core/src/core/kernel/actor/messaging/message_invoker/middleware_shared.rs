//! Shared wrapper for MessageInvokerMiddleware implementations.

use alloc::boxed::Box;

use fraktor_utils_core_rs::core::sync::{ArcShared, RuntimeRwLock, SharedAccess};

use super::middleware::MessageInvokerMiddleware;

/// Shared wrapper for [`MessageInvokerMiddleware`] implementations.
///
/// This wrapper provides [`SharedAccess`] methods (`with_read`/`with_write`)
/// that internally lock the underlying middleware, allowing safe
/// concurrent access from multiple owners.
pub(crate) struct MiddlewareShared {
  inner: ArcShared<RuntimeRwLock<Box<dyn MessageInvokerMiddleware>>>,
}

impl MiddlewareShared {
  /// Creates a new shared wrapper around the provided middleware.
  #[must_use]
  #[allow(dead_code)] // Used in tests
  pub(crate) fn new(middleware: Box<dyn MessageInvokerMiddleware>) -> Self {
    Self { inner: ArcShared::new(RuntimeRwLock::new(middleware)) }
  }
}

impl Clone for MiddlewareShared {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}

impl SharedAccess<Box<dyn MessageInvokerMiddleware>> for MiddlewareShared {
  fn with_read<R>(&self, f: impl FnOnce(&Box<dyn MessageInvokerMiddleware>) -> R) -> R {
    let guard = self.inner.read();
    f(&guard)
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut Box<dyn MessageInvokerMiddleware>) -> R) -> R {
    let mut guard = self.inner.write();
    f(&mut guard)
  }
}
