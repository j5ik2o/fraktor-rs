//! Listener adapter for [`ActorFuture`](crate::ActorFuture).

use core::{
  future::Future,
  pin::Pin,
  task::{Context, Poll},
};

use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdToolbox, RuntimeToolbox},
  sync::SharedAccess,
};

use super::ActorFutureSharedGeneric;

/// Future adapter that polls the underlying [`ActorFuture`](super::ActorFuture).
///
/// This listener holds a shared reference to the future and locks the mutex
/// on each poll to access the inner state.
pub struct ActorFutureListener<T, TB: RuntimeToolbox = NoStdToolbox>
where
  T: Send + 'static, {
  future: ActorFutureSharedGeneric<T, TB>,
}

impl<T, TB> ActorFutureListener<T, TB>
where
  T: Send + 'static,
  TB: RuntimeToolbox,
{
  /// Creates a new listener for the given shared future.
  #[must_use]
  pub const fn new(future: ActorFutureSharedGeneric<T, TB>) -> Self {
    Self { future }
  }
}

impl<T, TB> Clone for ActorFutureListener<T, TB>
where
  T: Send + 'static,
  TB: RuntimeToolbox,
{
  fn clone(&self) -> Self {
    Self { future: self.future.clone() }
  }
}

impl<T, TB> Future for ActorFutureListener<T, TB>
where
  T: Send + 'static,
  TB: RuntimeToolbox,
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

impl<T, TB> Unpin for ActorFutureListener<T, TB>
where
  T: Send + 'static,
  TB: RuntimeToolbox,
{
}
