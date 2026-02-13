//! Future specialized for mailbox user queue offers.

use core::{
  fmt,
  future::Future,
  pin::Pin,
  task::{Context, Poll},
  time::Duration,
};

use fraktor_utils_rs::core::{
  collections::{
    queue::{OfferOutcome, QueueError},
    wait::WaitShared,
  },
  runtime_toolbox::{NoStdToolbox, RuntimeToolbox, ToolboxMutex},
  sync::{ArcShared, sync_mutex_like::SyncMutexLike},
  timing::delay::{DelayFuture, DelayProvider},
};

use super::{mailbox_queue_state::QueueState, map_user_queue_error};
use crate::core::{error::SendError, messaging::AnyMessageGeneric};

#[cfg(test)]
mod tests;

/// Future returned when a queue needs to wait for capacity.
struct QueueOfferFuture<T, TB: RuntimeToolbox>
where
  T: Send + 'static, {
  state:   ArcShared<ToolboxMutex<QueueState<T, TB>, TB>>,
  message: Option<T>,
  waiter:  Option<WaitShared<QueueError<T>, TB>>,
  timeout: Option<DelayFuture>,
}

impl<T, TB: RuntimeToolbox> QueueOfferFuture<T, TB>
where
  T: Send + 'static,
{
  pub(crate) const fn new(state: ArcShared<ToolboxMutex<QueueState<T, TB>, TB>>, message: T) -> Self {
    Self { state, message: Some(message), waiter: None, timeout: None }
  }

  pub(crate) fn with_timeout(mut self, duration: Duration, provider: &mut dyn DelayProvider) -> Self {
    self.timeout = Some(provider.delay(duration));
    self
  }

  fn ensure_waiter(&mut self) -> Result<&mut WaitShared<QueueError<T>, TB>, QueueError<T>> {
    if self.waiter.is_none() {
      let waiter = {
        let mut state = self.state.lock();
        state.register_producer_waiter().map_err(|_| QueueError::Disconnected)?
      };
      self.waiter = Some(waiter);
    }
    // 安全性: 上のチェックで waiter が Some であることが保証される。
    Ok(unsafe { self.waiter.as_mut().unwrap_unchecked() })
  }
}

impl<T, TB> Unpin for QueueOfferFuture<T, TB>
where
  T: Send + 'static,
  TB: RuntimeToolbox,
{
}

impl<T, TB> Future for QueueOfferFuture<T, TB>
where
  T: Send + 'static,
  TB: RuntimeToolbox,
{
  type Output = Result<OfferOutcome, QueueError<T>>;

  fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
    let this = self.get_mut();
    loop {
      if this.timeout.as_mut().is_some_and(|timeout| Pin::new(timeout).poll(cx).is_ready()) {
        this.waiter.take();
        let message = unsafe {
          debug_assert!(this.message.is_some(), "timeout without pending message");
          this.message.take().unwrap_unchecked()
        };
        return Poll::Ready(Err(QueueError::TimedOut(message)));
      }

      if let Some(message) = this.message.take() {
        let mut state = this.state.lock();
        match state.offer(message) {
          | Ok(outcome) => {
            this.waiter.take();
            this.timeout = None;
            return Poll::Ready(Ok(outcome));
          },
          | Err(QueueError::Full(item)) => {
            this.message = Some(item);
          },
          | Err(error) => {
            this.waiter.take();
            return Poll::Ready(Err(error));
          },
        }
      }

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
    }
  }
}

/// Future completing once a user message has been enqueued.
pub struct MailboxOfferFutureGeneric<TB: RuntimeToolbox + 'static> {
  inner: QueueOfferFuture<AnyMessageGeneric<TB>, TB>,
}

/// Type alias for [MailboxOfferFutureGeneric] with the default [NoStdToolbox].
pub type MailboxOfferFuture = MailboxOfferFutureGeneric<NoStdToolbox>;

impl<TB: RuntimeToolbox + 'static> MailboxOfferFutureGeneric<TB> {
  pub(crate) const fn new(
    state: ArcShared<ToolboxMutex<QueueState<AnyMessageGeneric<TB>, TB>, TB>>,
    message: AnyMessageGeneric<TB>,
  ) -> Self {
    Self { inner: QueueOfferFuture::new(state, message) }
  }

  /// Configures the offer future to fail with a timeout if the duration elapses before enqueue
  /// succeeds.
  #[must_use]
  pub fn with_timeout(mut self, duration: Duration, provider: &mut dyn DelayProvider) -> Self {
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
