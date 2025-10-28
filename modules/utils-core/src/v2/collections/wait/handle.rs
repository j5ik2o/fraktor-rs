use core::{
  future::Future,
  pin::Pin,
  task::{Context, Poll},
};

use super::node::WaitNode;
use crate::sync::ArcShared;

/// Future returned when registering interest in a queue/stack event.
pub struct WaitHandle<E> {
  node: ArcShared<WaitNode<E>>,
}

impl<E> WaitHandle<E> {
  /// Creates a wait handle bound to the supplied waiter node.
  pub fn new(node: ArcShared<WaitNode<E>>) -> Self {
    Self { node }
  }

  fn node(&self) -> &WaitNode<E> {
    &self.node
  }
}

impl<E> Future for WaitHandle<E> {
  type Output = Result<(), E>;

  fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
    let this = self.get_mut();

    match this.node().poll(cx) {
      | Poll::Ready(()) => {
        let result = this.node().take_result().expect("completed waiter must hold a result");
        Poll::Ready(result)
      },
      | Poll::Pending => Poll::Pending,
    }
  }
}

impl<E> Drop for WaitHandle<E> {
  fn drop(&mut self) {
    self.node.cancel();
  }
}

impl<E> Clone for WaitHandle<E> {
  fn clone(&self) -> Self {
    Self { node: self.node.clone() }
  }
}
