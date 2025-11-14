//! Future specialized for mailbox user queue offers.

use core::{
  fmt,
  future::Future,
  pin::Pin,
  task::{Context, Poll},
  time::Duration,
};

use fraktor_utils_core_rs::{sync::NoStdToolbox};
use fraktor_utils_core_rs::timing::DelayProvider;
use super::{mailbox_queue_offer_future::QueueOfferFuture, map_user_queue_error};
use crate::{RuntimeToolbox, error::SendError, messaging::AnyMessageGeneric};

#[cfg(test)]
mod tests;

/// Future completing once a user message has been enqueued.
pub struct MailboxOfferFutureGeneric<TB: RuntimeToolbox + 'static> {
  inner: QueueOfferFuture<AnyMessageGeneric<TB>, TB>,
}

/// Type alias for [MailboxOfferFutureGeneric] with the default [NoStdToolbox].
pub type MailboxOfferFuture = MailboxOfferFutureGeneric<NoStdToolbox>;

impl<TB: RuntimeToolbox + 'static> MailboxOfferFutureGeneric<TB> {
  pub(super) const fn new(inner: QueueOfferFuture<AnyMessageGeneric<TB>, TB>) -> Self {
    Self { inner }
  }

  /// Configures the offer future to fail with a timeout if the duration elapses before enqueue
  /// succeeds.
  #[must_use]
  pub fn with_timeout(mut self, duration: Duration, provider: &dyn DelayProvider) -> Self {
    self.inner = self.inner.with_timeout(duration, provider);
    self
  }
}

impl<TB: RuntimeToolbox + 'static> Unpin for MailboxOfferFutureGeneric<TB> {}

impl<TB: RuntimeToolbox + 'static> Future for MailboxOfferFutureGeneric<TB> {
  type Output = Result<(), SendError<TB>>;

  fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
    match Pin::new(&mut self.inner).poll(cx) {
      | Poll::Ready(Ok(_)) => Poll::Ready(Ok(())),
      | Poll::Ready(Err(error)) => Poll::Ready(Err(map_user_queue_error(error))),
      | Poll::Pending => Poll::Pending,
    }
  }
}

impl<TB: RuntimeToolbox> fmt::Debug for MailboxOfferFutureGeneric<TB> {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("MailboxOfferFuture").finish()
  }
}
