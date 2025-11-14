use core::{
  future::Future,
  pin::Pin,
  task::{Context, Poll},
  time::Duration,
};

use super::{DequeEdge, DequeState};
use crate::{
  collections::{
    queue::{OfferOutcome, QueueError},
    wait::WaitShared,
  },
  runtime_toolbox::{NoStdToolbox, RuntimeToolbox},
  sync::ArcShared,
  timing::{DelayFuture, DelayProvider},
};

/// Future returned when a deque needs to wait for capacity.
pub struct DequeOfferFuture<T, TB: RuntimeToolbox + 'static = NoStdToolbox>
where
  T: Send + 'static, {
  state:   ArcShared<DequeState<T, TB>>,
  item:    Option<T>,
  waiter:  Option<WaitShared<QueueError<T>>>,
  edge:    DequeEdge,
  timeout: Option<DelayFuture>,
}

impl<T, TB> DequeOfferFuture<T, TB>
where
  T: Send + 'static,
  TB: RuntimeToolbox + 'static,
{
  pub(super) const fn new(state: ArcShared<DequeState<T, TB>>, item: T, edge: DequeEdge) -> Self {
    Self { state, item: Some(item), waiter: None, edge, timeout: None }
  }

  fn ensure_waiter(&mut self) -> &mut WaitShared<QueueError<T>> {
    if self.waiter.is_none() {
      let waiter = self.state.register_producer_waiter();
      self.waiter = Some(waiter);
    }
    // SAFETY: waiter is always populated after registration.
    unsafe { self.waiter.as_mut().unwrap_unchecked() }
  }

  /// Configures the future to fail with [`QueueError::TimedOut`] if capacity does not free up
  /// before the specified duration.
  #[must_use]
  pub fn with_timeout(mut self, duration: Duration, provider: &dyn DelayProvider) -> Self {
    self.timeout = Some(provider.delay(duration));
    self
  }
}

impl<T, TB> Future for DequeOfferFuture<T, TB>
where
  T: Send + 'static,
  TB: RuntimeToolbox + 'static,
{
  type Output = Result<OfferOutcome, QueueError<T>>;

  fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
    let this = self.get_mut();
    loop {
      let timeout_elapsed = this.timeout.as_mut().is_some_and(|timeout| Pin::new(timeout).poll(cx).is_ready());
      if timeout_elapsed {
        this.waiter.take();
        let item = unsafe {
          debug_assert!(this.item.is_some(), "timeout fired without pending value");
          this.item.take().unwrap_unchecked()
        };
        return Poll::Ready(Err(QueueError::TimedOut(item)));
      }

      if let Some(item) = this.item.take() {
        match this.state.offer(item, this.edge) {
          | Ok(outcome) => {
            this.waiter.take();
            this.timeout = None;
            return Poll::Ready(Ok(outcome));
          },
          | Err(QueueError::Full(returned)) => this.item = Some(returned),
          | Err(error) => {
            this.waiter.take();
            return Poll::Ready(Err(error));
          },
        }
      }

      let waiter = this.ensure_waiter();
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

impl<T, TB> Unpin for DequeOfferFuture<T, TB>
where
  T: Send + 'static,
  TB: RuntimeToolbox + 'static,
{
}
