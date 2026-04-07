//! Listener adapter for [`ActorFuture`](crate::ActorFuture).

use core::{
  future::Future,
  pin::Pin,
  task::{Context, Poll},
};

use fraktor_utils_core_rs::core::sync::SharedAccess;

use super::ActorFutureShared;

/// Future adapter that polls the underlying [`ActorFuture`](super::ActorFuture).
///
/// This listener holds a shared reference to the future and locks the mutex
/// on each poll to access the inner state.
pub struct ActorFutureListener<T>
where
  T: Send + 'static, {
  future: ActorFutureShared<T>,
}

impl<T> ActorFutureListener<T>
where
  T: Send + 'static,
{
  /// Creates a new listener for the given shared future.
  #[must_use]
  pub const fn new(future: ActorFutureShared<T>) -> Self {
    Self { future }
  }
}

impl<T> Clone for ActorFutureListener<T>
where
  T: Send + 'static,
{
  fn clone(&self) -> Self {
    Self { future: self.future.clone() }
  }
}

impl<T> Future for ActorFutureListener<T>
where
  T: Send + 'static,
{
  type Output = T;

  fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
    self.future.with_write(|inner| {
      if let Some(value) = inner.try_take() {
        Poll::Ready(value)
      } else {
        inner.register_waker(cx.waker());
        Poll::Pending
      }
    })
  }
}

impl<T> Unpin for ActorFutureListener<T> where T: Send + 'static {}
