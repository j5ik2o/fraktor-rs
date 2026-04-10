//! Future monitoring the user queue for incoming messages.

use core::{
  fmt::{Debug, Formatter, Result as FmtResult},
  future::Future,
  pin::Pin,
  task::{Context, Poll},
};

use fraktor_utils_core_rs::core::{
  collections::{queue::QueueError, wait::WaitShared},
  sync::{SharedAccess, SharedLock},
};

use super::{mailbox_queue_state::QueueState, map_user_queue_error};
use crate::core::kernel::actor::{error::SendError, messaging::AnyMessage};

#[cfg(test)]
mod tests;

/// Future resolving when a message becomes available in the queue.
struct QueuePollFuture<T>
where
  T: Send + 'static, {
  state:  SharedLock<QueueState<T>>,
  waiter: Option<WaitShared<QueueError<T>>>,
}

impl<T> QueuePollFuture<T>
where
  T: Send + 'static,
{
  fn ensure_waiter(&mut self) -> Result<&mut WaitShared<QueueError<T>>, QueueError<T>> {
    if self.waiter.is_none() {
      let waiter =
        self.state.with_write(|state| state.register_consumer_waiter().map_err(|_| QueueError::Disconnected))?;
      self.waiter = Some(waiter);
    }
    // 安全性: 上のチェックで waiter が Some であることが保証される。
    Ok(unsafe { self.waiter.as_mut().unwrap_unchecked() })
  }
}

impl<T> Unpin for QueuePollFuture<T> where T: Send + 'static {}

impl<T> Future for QueuePollFuture<T>
where
  T: Send + 'static,
{
  type Output = Result<T, QueueError<T>>;

  fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
    let this = self.get_mut();
    loop {
      let poll_result = this.state.with_write(|state| state.poll());
      match poll_result {
        | Ok(item) => {
          this.waiter.take();
          return Poll::Ready(Ok(item));
        },
        | Err(QueueError::Empty) => {
          let waiter = match this.ensure_waiter() {
            | Ok(w) => w,
            | Err(error) => {
              this.waiter.take();
              return Poll::Ready(Err(error));
            },
          };
          match Pin::new(waiter).poll(cx) {
            | Poll::Pending => return Poll::Pending,
            | Poll::Ready(Ok(())) => continue,
            | Poll::Ready(Err(error)) => {
              this.waiter.take();
              return Poll::Ready(Err(error));
            },
          }
        },
        | Err(error) => {
          this.waiter.take();
          return Poll::Ready(Err(error));
        },
      }
    }
  }
}

/// Future completing with the next user message from the mailbox.
pub struct MailboxPollFuture {
  inner: QueuePollFuture<AnyMessage>,
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

impl Debug for MailboxPollFuture {
  fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
    f.debug_struct("MailboxPollFuture").finish()
  }
}
