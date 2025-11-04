use core::{
  future::Future,
  pin::Pin,
  task::{Context, Poll},
};

use super::node::WaitNode;
use crate::sync::ArcShared;

/// Future returned when registering interest in a queue/stack event.
pub struct WaitShared<E> {
  node: ArcShared<WaitNode<E>>,
}

impl<E> WaitShared<E> {
  /// Creates a shared wait future bound to the supplied waiter node.
  #[must_use]
  pub const fn new(node: ArcShared<WaitNode<E>>) -> Self {
    Self { node }
  }

  fn node(&self) -> &WaitNode<E> {
    &self.node
  }
}

impl<E> Future for WaitShared<E> {
  type Output = Result<(), E>;

  fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
    let this = self.get_mut();

    match this.node().poll(cx) {
      | Poll::Ready(()) => {
        let result = this.node().take_result().unwrap_or_else(|| {
          debug_assert!(false, "Completed waiter must provide a result");
          Ok(())
        });
        Poll::Ready(result)
      },
      | Poll::Pending => Poll::Pending,
    }
  }
}

impl<E> Drop for WaitShared<E> {
  fn drop(&mut self) {
    self.node.cancel();
  }
}

impl<E> Clone for WaitShared<E> {
  fn clone(&self) -> Self {
    Self { node: self.node.clone() }
  }
}
