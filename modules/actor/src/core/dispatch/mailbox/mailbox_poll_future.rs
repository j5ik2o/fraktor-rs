//! Future monitoring the user queue for incoming messages.

use core::{
  fmt,
  future::Future,
  pin::Pin,
  task::{Context, Poll},
};

use fraktor_utils_rs::core::{
  collections::{queue::QueueError, wait::WaitShared},
  runtime_toolbox::{NoStdToolbox, RuntimeToolbox, ToolboxMutex},
  sync::{ArcShared, sync_mutex_like::SyncMutexLike},
};

use super::{mailbox_queue_state::QueueState, map_user_queue_error};
use crate::core::{error::SendError, messaging::AnyMessageGeneric};

#[cfg(test)]
mod tests;

/// Future resolving when a message becomes available in the queue.
struct QueuePollFuture<T, TB: RuntimeToolbox>
where
  T: Send + 'static, {
  state:  ArcShared<ToolboxMutex<QueueState<T, TB>, TB>>,
  waiter: Option<WaitShared<QueueError<T>, TB>>,
}

impl<T, TB: RuntimeToolbox> QueuePollFuture<T, TB>
where
  T: Send + 'static,
{
  fn ensure_waiter(&mut self) -> Result<&mut WaitShared<QueueError<T>, TB>, QueueError<T>> {
    if self.waiter.is_none() {
      let waiter = {
        let mut state = self.state.lock();
        state.register_consumer_waiter().map_err(|_| QueueError::Disconnected)?
      };
      self.waiter = Some(waiter);
    }
    // 安全性: 上のチェックで waiter が Some であることが保証される。
    Ok(unsafe { self.waiter.as_mut().unwrap_unchecked() })
  }
}

impl<T, TB> Unpin for QueuePollFuture<T, TB>
where
  T: Send + 'static,
  TB: RuntimeToolbox,
{
}

impl<T, TB> Future for QueuePollFuture<T, TB>
where
  T: Send + 'static,
  TB: RuntimeToolbox,
{
  type Output = Result<T, QueueError<T>>;

  fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
    let this = self.get_mut();
    loop {
      let poll_result = {
        let mut state = this.state.lock();
        state.poll()
      };
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
pub struct MailboxPollFutureGeneric<TB: RuntimeToolbox + 'static> {
  inner: QueuePollFuture<AnyMessageGeneric<TB>, TB>,
}

/// Type alias for [MailboxPollFutureGeneric] with the default [NoStdToolbox].
pub type MailboxPollFuture = MailboxPollFutureGeneric<NoStdToolbox>;

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
