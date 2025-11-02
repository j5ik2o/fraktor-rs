use core::{
  fmt,
  future::Future,
  pin::Pin,
  task::{Context, Poll},
};

use super::queue_poll_future::QueuePollFuture;
use crate::{any_message::AnyMessage, mailbox::map_user_queue_error, send_error::SendError};

/// Future specialized for mailbox user queue polling.
pub struct MailboxPollFuture {
  inner: QueuePollFuture<AnyMessage>,
}

impl MailboxPollFuture {
  pub(super) const fn new(inner: QueuePollFuture<AnyMessage>) -> Self {
    Self { inner }
  }
}

impl Unpin for MailboxPollFuture {}

impl Future for MailboxPollFuture {
  type Output = Result<AnyMessage, SendError>;

  fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
    match Pin::new(&mut self.inner).poll(cx) {
      | Poll::Ready(Ok(message)) => Poll::Ready(Ok(message)),
      | Poll::Ready(Err(error)) => Poll::Ready(Err(map_user_queue_error(error))),
      | Poll::Pending => Poll::Pending,
    }
  }
}

impl fmt::Debug for MailboxPollFuture {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("MailboxPollFuture").finish()
  }
}
