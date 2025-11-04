//! Future returned when a queue needs to wait for incoming messages.

use core::{
  future::Future,
  pin::Pin,
  task::{Context, Poll},
};

use cellactor_utils_core_rs::{
  collections::{queue::QueueError, wait::WaitShared},
  sync::ArcShared,
};

use super::mailbox_queue_state::QueueState;
use crate::RuntimeToolbox;

/// Future resolving when a message becomes available in the queue.
pub struct QueuePollFuture<T, TB: RuntimeToolbox>
where
  T: Send + 'static, {
  state:  ArcShared<QueueState<T, TB>>,
  waiter: Option<WaitShared<QueueError<T>>>,
}

impl<T, TB: RuntimeToolbox> QueuePollFuture<T, TB>
where
  T: Send + 'static,
{
  pub(super) const fn new(state: ArcShared<QueueState<T, TB>>) -> Self {
    Self { state, waiter: None }
  }

  fn ensure_waiter(&mut self) -> &mut WaitShared<QueueError<T>> {
    if self.waiter.is_none() {
      let waiter = self.state.register_consumer_waiter();
      self.waiter = Some(waiter);
    }
    // SAFETY: waiter is guaranteed to be Some after the above check.
    unsafe { self.waiter.as_mut().unwrap_unchecked() }
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
