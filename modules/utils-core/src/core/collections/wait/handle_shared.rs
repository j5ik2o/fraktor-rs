//! Shared wait handle for async collection operations.

use core::{
  future::Future,
  pin::Pin,
  task::{Context, Poll},
};

use super::WaitNodeShared;
use crate::core::sync::SharedAccess;

/// Future returned when registering interest in a queue/stack event.
pub struct WaitShared<E: Send + 'static> {
  node: WaitNodeShared<E>,
}

impl<E: Send + 'static> WaitShared<E> {
  /// Creates a shared wait future bound to the supplied waiter node.
  #[must_use]
  pub const fn new(node: WaitNodeShared<E>) -> Self {
    Self { node }
  }
}

impl<E: Send + 'static> Future for WaitShared<E> {
  type Output = Result<(), E>;

  fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
    let node = &self.as_ref().get_ref().node;

    node.with_write(|guard| match guard.poll(cx) {
      | Poll::Ready(()) => {
        let result = guard.take_result().unwrap_or_else(|| {
          debug_assert!(false, "Completed waiter must provide a result");
          Ok(())
        });
        Poll::Ready(result)
      },
      | Poll::Pending => Poll::Pending,
    })
  }
}

impl<E: Send + 'static> Drop for WaitShared<E> {
  fn drop(&mut self) {
    self.node.with_write(|n| n.cancel());
  }
}

impl<E: Send + 'static> Clone for WaitShared<E> {
  fn clone(&self) -> Self {
    Self { node: self.node.clone() }
  }
}
