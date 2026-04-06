//! Async future for awaiting actor system termination.

use core::{
  future::Future,
  pin::Pin,
  task::{Context, Poll},
};

use super::TerminationSignal;

/// Future that resolves when the actor system terminates.
///
/// Obtained via [`TerminationSignal::into_future()`](core::future::IntoFuture)
/// or equivalently by calling `.await` on a [`TerminationSignal`].
pub struct TerminationFuture {
  signal: TerminationSignal,
}

impl TerminationFuture {
  /// Creates a future from the given signal.
  pub(crate) const fn new(signal: TerminationSignal) -> Self {
    Self { signal }
  }
}

impl Future for TerminationFuture {
  type Output = ();

  fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
    if self.signal.is_terminated() {
      return Poll::Ready(());
    }
    self.signal.register_waker(cx.waker());
    // Double-check after registration to avoid lost wakeups.
    if self.signal.is_terminated() { Poll::Ready(()) } else { Poll::Pending }
  }
}

impl Unpin for TerminationFuture {}
