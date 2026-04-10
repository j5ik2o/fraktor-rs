//! Shared wrapper for WaitNode enabling interior mutability.

use super::node::WaitNode;
use crate::core::sync::{SharedAccess, SharedLock, SpinSyncMutex};

/// Shared wrapper for [`WaitNode`] enabling interior mutability.
///
/// This wrapper provides [`SharedAccess`] methods (`with_read`/`with_write`)
/// that internally lock the underlying [`WaitNode`], allowing safe
/// concurrent access from multiple owners.
pub struct WaitNodeShared<E: Send + 'static> {
  inner: SharedLock<WaitNode<E>>,
}

impl<E: Send + 'static> WaitNodeShared<E> {
  /// Creates a new shared wrapper around a fresh WaitNode.
  #[must_use]
  pub fn new() -> Self {
    Self { inner: SharedLock::new_with_driver::<SpinSyncMutex<_>>(WaitNode::new()) }
  }
}

impl<E: Send + 'static> Default for WaitNodeShared<E> {
  fn default() -> Self {
    Self::new()
  }
}

impl<E: Send + 'static> Clone for WaitNodeShared<E> {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}

impl<E: Send + 'static> SharedAccess<WaitNode<E>> for WaitNodeShared<E> {
  fn with_read<R>(&self, f: impl FnOnce(&WaitNode<E>) -> R) -> R {
    self.inner.with_read(f)
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut WaitNode<E>) -> R) -> R {
    self.inner.with_write(f)
  }
}
