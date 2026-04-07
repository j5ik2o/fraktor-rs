//! Shared wrapper for MessageInvoker implementations.

use alloc::boxed::Box;

use fraktor_utils_rs::core::sync::{ArcShared, RuntimeRwLock, SharedAccess};

use super::invoker_trait::MessageInvoker;

/// Shared wrapper for [`MessageInvoker`] implementations.
///
/// This wrapper provides [`SharedAccess`] methods (`with_read`/`with_write`)
/// that internally lock the underlying invoker, allowing safe
/// concurrent access from multiple owners.
pub struct MessageInvokerShared {
  inner: ArcShared<RuntimeRwLock<Box<dyn MessageInvoker>>>,
}

impl MessageInvokerShared {
  /// Creates a new shared wrapper around the provided invoker.
  #[must_use]
  pub fn new(invoker: Box<dyn MessageInvoker>) -> Self {
    Self { inner: ArcShared::new(RuntimeRwLock::new(invoker)) }
  }
}

impl Clone for MessageInvokerShared {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}

impl SharedAccess<Box<dyn MessageInvoker>> for MessageInvokerShared {
  fn with_read<R>(&self, f: impl FnOnce(&Box<dyn MessageInvoker>) -> R) -> R {
    let guard = self.inner.read();
    f(&guard)
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut Box<dyn MessageInvoker>) -> R) -> R {
    let mut guard = self.inner.write();
    f(&mut guard)
  }
}
