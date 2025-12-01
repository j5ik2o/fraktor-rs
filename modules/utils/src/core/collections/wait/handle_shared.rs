use core::{
  future::Future,
  pin::Pin,
  task::{Context, Poll},
};

use super::{WaitNodeShared, node::WaitNode};
use crate::core::{
  runtime_toolbox::{NoStdToolbox, RuntimeToolbox},
  sync::sync_mutex_like::SyncMutexLike,
};

/// Future returned when registering interest in a queue/stack event.
pub struct WaitShared<E: Send + 'static, TB: RuntimeToolbox = NoStdToolbox> {
  node: WaitNodeShared<E, TB>,
}

impl<E: Send + 'static, TB> WaitShared<E, TB>
where
  TB: RuntimeToolbox + 'static,
{
  /// Creates a shared wait future bound to the supplied waiter node.
  #[must_use]
  pub const fn new(node: WaitNodeShared<E, TB>) -> Self {
    Self { node }
  }

  #[allow(clippy::missing_const_for_fn)] // ArcShared の Deref が const でないため const fn 化できない
  fn node(&self) -> &crate::core::runtime_toolbox::ToolboxMutex<WaitNode<E>, TB> {
    &self.node
  }
}

impl<E: Send + 'static, TB> Future for WaitShared<E, TB>
where
  TB: RuntimeToolbox + 'static,
{
  type Output = Result<(), E>;

  fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
    let this = self.get_mut();

    let mut guard = this.node().lock();

    match guard.poll(cx) {
      | Poll::Ready(()) => {
        let result = guard.take_result().unwrap_or_else(|| {
          debug_assert!(false, "Completed waiter must provide a result");
          Ok(())
        });
        Poll::Ready(result)
      },
      | Poll::Pending => Poll::Pending,
    }
  }
}

impl<E: Send + 'static, TB: RuntimeToolbox> Drop for WaitShared<E, TB> {
  fn drop(&mut self) {
    self.node.lock().cancel();
  }
}

impl<E: Send + 'static, TB: RuntimeToolbox> Clone for WaitShared<E, TB> {
  fn clone(&self) -> Self {
    Self { node: self.node.clone() }
  }
}
