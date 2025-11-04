//! Listener adapter for [`ActorFuture`](crate::ActorFuture).

use core::{
  future::Future,
  pin::Pin,
  task::{Context, Poll},
};

use crate::{NoStdToolbox, RuntimeToolbox, futures::ActorFuture};

/// Future adapter that polls the underlying [`ActorFuture`].
pub struct ActorFutureListener<'a, T, TB: RuntimeToolbox = NoStdToolbox>
where
  T: Send + 'static, {
  future: &'a ActorFuture<T, TB>,
}

impl<'a, T, TB> ActorFutureListener<'a, T, TB>
where
  T: Send + 'static,
  TB: RuntimeToolbox,
{
  pub(crate) const fn new(future: &'a ActorFuture<T, TB>) -> Self {
    Self { future }
  }
}

impl<'a, T, TB> Future for ActorFutureListener<'a, T, TB>
where
  T: Send + 'static,
  TB: RuntimeToolbox,
{
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

impl<'a, T, TB> Unpin for ActorFutureListener<'a, T, TB>
where
  T: Send + 'static,
  TB: RuntimeToolbox,
{
}
