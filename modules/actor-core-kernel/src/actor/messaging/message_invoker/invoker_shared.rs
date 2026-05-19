//! Shared wrapper for MessageInvoker implementations.

use alloc::boxed::Box;

use fraktor_utils_core_rs::sync::{DefaultRwLock, SharedAccess, SharedRwLock};

use super::invoker_trait::MessageInvoker;

/// Shared wrapper for [`MessageInvoker`] implementations.
///
/// This wrapper provides [`SharedAccess`] methods (`with_read`/`with_write`)
/// that serialize access to the underlying invoker, allowing safe concurrent
/// access from multiple owners.
#[derive(Clone)]
pub struct MessageInvokerShared {
  inner: SharedRwLock<Box<dyn MessageInvoker>>,
}

impl MessageInvokerShared {
  /// Creates a shared wrapper around the provided invoker.
  #[must_use]
  pub fn new(invoker: Box<dyn MessageInvoker>) -> Self {
    Self { inner: SharedRwLock::new_with_driver::<DefaultRwLock<_>>(invoker) }
  }
}

impl SharedAccess<Box<dyn MessageInvoker>> for MessageInvokerShared {
  fn with_read<R>(&self, f: impl FnOnce(&Box<dyn MessageInvoker>) -> R) -> R {
    self.inner.with_read(f)
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut Box<dyn MessageInvoker>) -> R) -> R {
    self.inner.with_write(f)
  }
}
