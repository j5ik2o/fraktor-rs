//! Future returned when a queue needs to wait for capacity.

use core::{
  future::Future,
  pin::Pin,
  task::{Context, Poll},
  time::Duration,
};

use fraktor_utils_core_rs::core::{
  collections::{
    queue::{QueueError, backend::OfferOutcome},
    wait::WaitShared,
  },
  sync::ArcShared,
  timing::{DelayFuture, DelayProvider},
};

use super::mailbox_queue_state::QueueState;
use crate::RuntimeToolbox;

/// Future completing when a queued message has been enqueued.
pub struct QueueOfferFuture<T, TB: RuntimeToolbox>
where
  T: Send + 'static, {
  state:   ArcShared<QueueState<T, TB>>,
  message: Option<T>,
  waiter:  Option<WaitShared<QueueError<T>>>,
  timeout: Option<DelayFuture>,
}

impl<T, TB: RuntimeToolbox> QueueOfferFuture<T, TB>
where
  T: Send + 'static,
{
  pub(super) const fn new(state: ArcShared<QueueState<T, TB>>, message: T) -> Self {
    Self { state, message: Some(message), waiter: None, timeout: None }
  }

  pub(super) fn with_timeout(mut self, duration: Duration, provider: &dyn DelayProvider) -> Self {
    self.timeout = Some(provider.delay(duration));
    self
  }

  fn ensure_waiter(&mut self) -> Result<&mut WaitShared<QueueError<T>>, QueueError<T>> {
    if self.waiter.is_none() {
      let waiter = self.state.register_producer_waiter().map_err(|_| QueueError::Disconnected)?;
      self.waiter = Some(waiter);
    }
    // SAFETY: waiter is guaranteed to be Some after the above check.
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
        match this.state.offer(message) {
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
