//! Future returned when a queue needs to wait for capacity.

use core::{
  future::Future,
  pin::Pin,
  task::{Context, Poll},
};

use cellactor_utils_core_rs::{
  collections::{
    queue::{QueueError, backend::OfferOutcome},
    wait::WaitHandle,
  },
  sync::ArcShared,
};

use super::mailbox_queue_state::QueueState;
use crate::RuntimeToolbox;

/// Future completing when a queued message has been enqueued.
pub struct QueueOfferFuture<T, TB: RuntimeToolbox>
where
  T: Send + 'static, {
  state:   ArcShared<QueueState<T, TB>>,
  message: Option<T>,
  waiter:  Option<WaitHandle<QueueError<T>>>,
}

impl<T, TB: RuntimeToolbox> QueueOfferFuture<T, TB>
where
  T: Send + 'static,
{
  pub(super) const fn new(state: ArcShared<QueueState<T, TB>>, message: T) -> Self {
    Self { state, message: Some(message), waiter: None }
  }

  fn ensure_waiter(&mut self) -> &mut WaitHandle<QueueError<T>> {
    if self.waiter.is_none() {
      let waiter = self.state.register_producer_waiter();
      self.waiter = Some(waiter);
    }
    // SAFETY: waiter is guaranteed to be Some after the above check.
    unsafe { self.waiter.as_mut().unwrap_unchecked() }
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
      if let Some(message) = this.message.take() {
        match this.state.offer(message) {
          | Ok(outcome) => {
            this.waiter.take();
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
