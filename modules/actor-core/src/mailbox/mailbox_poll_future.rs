//! Future monitoring the user queue for incoming messages.

use core::{
  fmt,
  future::Future,
  pin::Pin,
  task::{Context, Poll},
};

use fraktor_utils_core_rs::sync::NoStdToolbox;

use super::{mailbox_queue_poll_future::QueuePollFuture, map_user_queue_error};
use crate::{RuntimeToolbox, error::SendError, messaging::AnyMessageGeneric};

#[cfg(test)]
mod tests;

/// Future completing with the next user message from the mailbox.
pub struct MailboxPollFutureGeneric<TB: RuntimeToolbox + 'static> {
  inner: QueuePollFuture<AnyMessageGeneric<TB>, TB>,
}

/// Type alias for [MailboxPollFutureGeneric] with the default [NoStdToolbox].
pub type MailboxPollFuture = MailboxPollFutureGeneric<NoStdToolbox>;

impl<TB: RuntimeToolbox + 'static> MailboxPollFutureGeneric<TB> {
  #[allow(dead_code)]
  pub(super) const fn new(inner: QueuePollFuture<AnyMessageGeneric<TB>, TB>) -> Self {
    Self { inner }
  }
}

impl<TB: RuntimeToolbox + 'static> Unpin for MailboxPollFutureGeneric<TB> {}

impl<TB: RuntimeToolbox + 'static> Future for MailboxPollFutureGeneric<TB> {
  type Output = Result<AnyMessageGeneric<TB>, SendError<TB>>;

  fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
    match Pin::new(&mut self.inner).poll(cx) {
      | Poll::Ready(Ok(message)) => Poll::Ready(Ok(message)),
      | Poll::Ready(Err(error)) => Poll::Ready(Err(map_user_queue_error(error))),
      | Poll::Pending => Poll::Pending,
    }
  }
}

impl<TB: RuntimeToolbox> fmt::Debug for MailboxPollFutureGeneric<TB> {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("MailboxPollFuture").finish()
  }
}
