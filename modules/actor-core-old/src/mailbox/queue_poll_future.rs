use core::{
  future::Future,
  pin::Pin,
  task::{Context, Poll},
};

use cellactor_utils_core_rs::{
  collections::{queue::QueueError, wait::WaitHandle},
  sync::ArcShared,
};

use super::queue_state::QueueState;

/// Future returned when a queue needs to wait for incoming messages.
pub struct QueuePollFuture<T> {
  state:  ArcShared<QueueState<T>>,
  waiter: Option<WaitHandle<QueueError<T>>>,
}

impl<T> QueuePollFuture<T> {
  pub(super) const fn new(state: ArcShared<QueueState<T>>) -> Self {
    Self { state, waiter: None }
  }

  fn ensure_waiter(&mut self) -> &mut WaitHandle<QueueError<T>> {
    if self.waiter.is_none() {
      let waiter = self.state.register_consumer_waiter();
      self.waiter = Some(waiter);
    }
    // SAFETY: waiter is guaranteed to be Some after the above check
    unsafe { self.waiter.as_mut().unwrap_unchecked() }
  }
}

impl<T> Unpin for QueuePollFuture<T> {}

impl<T> Future for QueuePollFuture<T> {
  type Output = Result<T, QueueError<T>>;

  fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
    let this = self.get_mut();
    loop {
      match this.state.poll() {
        | Ok(item) => {
          this.waiter.take();
          return Poll::Ready(Ok(item));
        },
        | Err(QueueError::Empty) => {
          let waiter = this.ensure_waiter();
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
