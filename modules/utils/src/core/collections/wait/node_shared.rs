//! Shared wrapper for WaitNode enabling interior mutability.

use super::node::WaitNode;
use crate::core::sync::{ArcShared, RuntimeMutex, SharedAccess};

/// Shared wrapper for [`WaitNode`] enabling interior mutability.
///
/// This wrapper provides [`SharedAccess`] methods (`with_read`/`with_write`)
/// that internally lock the underlying [`WaitNode`], allowing safe
/// concurrent access from multiple owners.
pub struct WaitNodeShared<E: Send + 'static> {
  inner: ArcShared<RuntimeMutex<WaitNode<E>>>,
}

impl<E: Send + 'static> WaitNodeShared<E> {
  /// Creates a new shared wrapper around a fresh WaitNode.
  #[must_use]
  pub fn new() -> Self {
    Self { inner: ArcShared::new(RuntimeMutex::new(WaitNode::new())) }
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
    let guard = self.inner.lock();
    f(&guard)
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut WaitNode<E>) -> R) -> R {
    let mut guard = self.inner.lock();
    f(&mut guard)
  }
}
