//! Listener adapter for [`ActorFuture`](super::actor_future::ActorFuture).

use core::{
  future::Future,
  pin::Pin,
  task::{Context, Poll},
};

use crate::actor_future::ActorFuture;

/// Future adapter that polls the underlying [`ActorFuture`].
pub struct ActorFutureListener<'a, T> {
  future: &'a ActorFuture<T>,
}

impl<'a, T> ActorFutureListener<'a, T> {
  pub(super) const fn new(future: &'a ActorFuture<T>) -> Self {
    Self { future }
  }
}

impl<'a, T> Future for ActorFutureListener<'a, T> {
  type Output = T;

  fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
    if let Some(value) = self.future.try_take() {
      Poll::Ready(value)
    } else {
      self.future.register_waker(cx.waker());
      Poll::Pending
    }
  }
}

impl<'a, T> Unpin for ActorFutureListener<'a, T> {}
