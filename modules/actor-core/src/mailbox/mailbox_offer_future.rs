use core::{
  fmt,
  future::Future,
  pin::Pin,
  task::{Context, Poll},
};

use super::queue_offer_future::QueueOfferFuture;
use crate::{any_message::AnyMessage, mailbox::map_user_queue_error, send_error::SendError};

/// Future specialized for mailbox user queue offers.
pub struct MailboxOfferFuture {
  inner: QueueOfferFuture<AnyMessage>,
}

impl MailboxOfferFuture {
  pub(super) const fn new(inner: QueueOfferFuture<AnyMessage>) -> Self {
    Self { inner }
  }
}

impl Unpin for MailboxOfferFuture {}

impl Future for MailboxOfferFuture {
  type Output = Result<(), SendError>;

  fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
    match Pin::new(&mut self.inner).poll(cx) {
      | Poll::Ready(Ok(_)) => Poll::Ready(Ok(())),
      | Poll::Ready(Err(error)) => Poll::Ready(Err(map_user_queue_error(error))),
      | Poll::Pending => Poll::Pending,
    }
  }
}

impl fmt::Debug for MailboxOfferFuture {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("MailboxOfferFuture").finish()
  }
}
