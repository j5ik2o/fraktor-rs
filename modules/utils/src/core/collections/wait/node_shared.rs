//! Shared wrapper for WaitNode enabling interior mutability.

use super::node::WaitNode;
use crate::core::{
  runtime_toolbox::{NoStdToolbox, RuntimeToolbox, SyncMutexFamily, ToolboxMutex},
  sync::{ArcShared, SharedAccess, sync_mutex_like::SyncMutexLike},
};

/// Shared wrapper for [`WaitNode`] enabling interior mutability.
///
/// This wrapper provides [`SharedAccess`] methods (`with_read`/`with_write`)
/// that internally lock the underlying [`WaitNode`], allowing safe
/// concurrent access from multiple owners.
pub struct WaitNodeShared<E: Send + 'static, TB: RuntimeToolbox = NoStdToolbox> {
  inner: ArcShared<ToolboxMutex<WaitNode<E>, TB>>,
}

impl<E: Send + 'static, TB: RuntimeToolbox + 'static> WaitNodeShared<E, TB> {
  /// Creates a new shared wrapper around a fresh WaitNode.
  #[must_use]
  pub fn new() -> Self {
    Self { inner: ArcShared::new(<TB::MutexFamily as SyncMutexFamily>::create(WaitNode::new())) }
  }
}

impl<E: Send + 'static, TB: RuntimeToolbox + 'static> Default for WaitNodeShared<E, TB> {
  fn default() -> Self {
    Self::new()
  }
}

impl<E: Send + 'static, TB: RuntimeToolbox> Clone for WaitNodeShared<E, TB> {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}

impl<E: Send + 'static, TB: RuntimeToolbox + 'static> SharedAccess<WaitNode<E>> for WaitNodeShared<E, TB> {
  fn with_read<R>(&self, f: impl FnOnce(&WaitNode<E>) -> R) -> R {
    let guard = self.inner.lock();
    f(&guard)
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut WaitNode<E>) -> R) -> R {
    let mut guard = self.inner.lock();
    f(&mut guard)
  }
}
