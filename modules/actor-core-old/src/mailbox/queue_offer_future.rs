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

use super::queue_state::QueueState;

/// Future returned when a queue needs to wait for capacity.
pub struct QueueOfferFuture<T> {
  state:   ArcShared<QueueState<T>>,
  message: Option<T>,
  waiter:  Option<WaitHandle<QueueError<T>>>,
}

impl<T> QueueOfferFuture<T> {
  pub(super) const fn new(state: ArcShared<QueueState<T>>, message: T) -> Self {
    Self { state, message: Some(message), waiter: None }
  }

  fn ensure_waiter(&mut self) -> &mut WaitHandle<QueueError<T>> {
    if self.waiter.is_none() {
      let waiter = self.state.register_producer_waiter();
      self.waiter = Some(waiter);
    }
    // SAFETY: waiter is guaranteed to be Some after the above check
    unsafe { self.waiter.as_mut().unwrap_unchecked() }
  }
}

impl<T> Unpin for QueueOfferFuture<T> {}

impl<T> Future for QueueOfferFuture<T> {
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
