//! Future returned by [`TickExecutorSignal::wait_async`].

use core::{
  pin::Pin,
  task::{Context, Poll},
};

use super::TickExecutorSignal;

/// Future waiting for a notification from [`TickExecutorSignal`].
pub(crate) struct TickExecutorSignalFuture<'a> {
  pub(crate) signal: &'a TickExecutorSignal,
}

impl core::future::Future for TickExecutorSignalFuture<'_> {
  type Output = ();

  fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
    if self.signal.arm() {
      return Poll::Ready(());
    }
    self.signal.register_waker(cx.waker());
    if self.signal.arm() { Poll::Ready(()) } else { Poll::Pending }
  }
}
