//! Future specialized for mailbox user queue offers.

use core::{fmt, future::Future, pin::Pin, task::{Context, Poll}};

use super::mailbox_queue_offer_future::QueueOfferFuture;
use super::map_user_queue_error;
use crate::{any_message::AnyMessage, RuntimeToolbox, SendError};

/// Future completing once a user message has been enqueued.
pub struct MailboxOfferFuture<TB: RuntimeToolbox + 'static> {
  inner: QueueOfferFuture<AnyMessage<TB>, TB>,
}

impl<TB: RuntimeToolbox + 'static> MailboxOfferFuture<TB> {
  pub(super) const fn new(inner: QueueOfferFuture<AnyMessage<TB>, TB>) -> Self {
    Self { inner }
  }
}

impl<TB: RuntimeToolbox + 'static> Unpin for MailboxOfferFuture<TB> {}

impl<TB: RuntimeToolbox + 'static> Future for MailboxOfferFuture<TB> {
  type Output = Result<(), SendError<TB>>;

  fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
    match Pin::new(&mut self.inner).poll(cx) {
      | Poll::Ready(Ok(_)) => Poll::Ready(Ok(())),
      | Poll::Ready(Err(error)) => Poll::Ready(Err(map_user_queue_error(error))),
      | Poll::Pending => Poll::Pending,
    }
  }
}

impl<TB: RuntimeToolbox> fmt::Debug for MailboxOfferFuture<TB> {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("MailboxOfferFuture").finish()
  }
}
