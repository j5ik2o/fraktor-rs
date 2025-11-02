//! Future monitoring the user queue for incoming messages.

use core::{fmt, future::Future, pin::Pin, task::{Context, Poll}};

use super::mailbox_queue_poll_future::QueuePollFuture;
use super::map_user_queue_error;
use crate::{any_message::AnyMessage, RuntimeToolbox, SendError};

/// Future completing with the next user message from the mailbox.
pub struct MailboxPollFuture<TB: RuntimeToolbox + 'static> {
  inner: QueuePollFuture<AnyMessage<TB>, TB>,
}

impl<TB: RuntimeToolbox + 'static> MailboxPollFuture<TB> {
  pub(super) const fn new(inner: QueuePollFuture<AnyMessage<TB>, TB>) -> Self {
    Self { inner }
  }
}

impl<TB: RuntimeToolbox + 'static> Unpin for MailboxPollFuture<TB> {}

impl<TB: RuntimeToolbox + 'static> Future for MailboxPollFuture<TB> {
  type Output = Result<AnyMessage<TB>, SendError<TB>>;

  fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
    match Pin::new(&mut self.inner).poll(cx) {
      | Poll::Ready(Ok(message)) => Poll::Ready(Ok(message)),
      | Poll::Ready(Err(error)) => Poll::Ready(Err(map_user_queue_error(error))),
      | Poll::Pending => Poll::Pending,
    }
  }
}

impl<TB: RuntimeToolbox> fmt::Debug for MailboxPollFuture<TB> {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("MailboxPollFuture").finish()
  }
}
