//! Future specialized for mailbox user queue offers.

use core::{
  fmt,
  future::Future,
  pin::Pin,
  sync::atomic::{AtomicUsize, Ordering},
  task::{Context, Poll},
  time::Duration,
};

use fraktor_utils_rs::core::{
  collections::{
    queue::{OfferOutcome, QueueError},
    wait::WaitShared,
  },
  sync::{ArcShared, RuntimeMutex},
  timing::delay::{DelayFuture, DelayProvider},
};

use super::{mailbox_instrumentation::MailboxInstrumentation, mailbox_queue_state::QueueState, map_user_queue_error};
use crate::core::kernel::{error::SendError, messaging::AnyMessage};

#[cfg(test)]
mod tests;

/// Future returned when a queue needs to wait for capacity.
struct QueueOfferFuture<T>
where
  T: Send + 'static, {
  state:           ArcShared<RuntimeMutex<QueueState<T>>>,
  user_queue_lock: Option<ArcShared<RuntimeMutex<()>>>,
  message:         Option<T>,
  waiter:          Option<WaitShared<QueueError<T>>>,
  timeout:         Option<DelayFuture>,
}

impl<T> QueueOfferFuture<T>
where
  T: Send + 'static,
{
  pub(crate) const fn new(state: ArcShared<RuntimeMutex<QueueState<T>>>, message: T) -> Self {
    Self { state, user_queue_lock: None, message: Some(message), waiter: None, timeout: None }
  }

  pub(crate) fn with_timeout(mut self, duration: Duration, provider: &mut dyn DelayProvider) -> Self {
    self.timeout = Some(provider.delay(duration));
    self
  }

  pub(crate) fn with_user_queue_lock(mut self, user_queue_lock: ArcShared<RuntimeMutex<()>>) -> Self {
    self.user_queue_lock = Some(user_queue_lock);
    self
  }

  fn offer_message(&self, message: T) -> Result<OfferOutcome, QueueError<T>> {
    if let Some(user_queue_lock) = self.user_queue_lock.as_ref() {
      let _guard = user_queue_lock.lock();
      let mut state = self.state.lock();
      return state.offer(message);
    }
    let mut state = self.state.lock();
    state.offer(message)
  }

  fn ensure_waiter(&mut self) -> Result<&mut WaitShared<QueueError<T>>, QueueError<T>> {
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

impl<T> Unpin for QueueOfferFuture<T> where T: Send + 'static {}

impl<T> Future for QueueOfferFuture<T>
where
  T: Send + 'static,
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
        match this.offer_message(message) {
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
struct MailboxMetrics {
  instrumentation: ArcShared<RuntimeMutex<Option<MailboxInstrumentation>>>,
  system_len:      ArcShared<AtomicUsize>,
}

/// Future completing once a user message has been enqueued.
pub struct MailboxOfferFuture {
  inner:   QueueOfferFuture<AnyMessage>,
  metrics: Option<MailboxMetrics>,
}

impl MailboxOfferFuture {
  pub(crate) const fn new(state: ArcShared<RuntimeMutex<QueueState<AnyMessage>>>, message: AnyMessage) -> Self {
    Self { inner: QueueOfferFuture::new(state, message), metrics: None }
  }

  pub(crate) fn with_user_queue_lock(mut self, user_queue_lock: ArcShared<RuntimeMutex<()>>) -> Self {
    self.inner = self.inner.with_user_queue_lock(user_queue_lock);
    self
  }

  pub(crate) fn with_metrics(
    mut self,
    instrumentation: ArcShared<RuntimeMutex<Option<MailboxInstrumentation>>>,
    system_len: ArcShared<AtomicUsize>,
  ) -> Self {
    self.metrics = Some(MailboxMetrics { instrumentation, system_len });
    self
  }

  fn publish_metrics(&self) {
    let Some(metrics) = self.metrics.as_ref() else {
      return;
    };
    let user_len = {
      let state = self.inner.state.lock();
      state.len()
    };
    let guard = metrics.instrumentation.lock();
    if let Some(instrumentation) = guard.as_ref() {
      instrumentation.publish(user_len, metrics.system_len.load(Ordering::Acquire));
    }
  }

  /// Configures the offer future to fail with a timeout if the duration elapses before enqueue
  /// succeeds.
  #[must_use]
  pub fn with_timeout(mut self, duration: Duration, provider: &mut dyn DelayProvider) -> Self {
    self.inner = self.inner.with_timeout(duration, provider);
    self
  }
}

impl Unpin for MailboxOfferFuture {}

impl Future for MailboxOfferFuture {
  type Output = Result<(), SendError>;

  fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
    match Pin::new(&mut self.inner).poll(cx) {
      | Poll::Ready(Ok(_)) => {
        self.publish_metrics();
        Poll::Ready(Ok(()))
      },
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
