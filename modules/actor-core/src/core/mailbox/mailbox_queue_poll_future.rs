//! Future returned when a queue needs to wait for incoming messages.

use core::{
  future::Future,
  pin::Pin,
  task::{Context, Poll},
};

use fraktor_utils_rs::core::{
  collections::{queue::QueueError, wait::WaitShared},
  runtime_toolbox::RuntimeToolbox,
  sync::ArcShared,
};

use super::mailbox_queue_state::QueueState;

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
  #[allow(dead_code)]
  pub(crate) const fn new(state: ArcShared<QueueState<T, TB>>) -> Self {
    Self { state, waiter: None }
  }

  fn ensure_waiter(&mut self) -> Result<&mut WaitShared<QueueError<T>>, QueueError<T>> {
    if self.waiter.is_none() {
      let waiter = self.state.register_consumer_waiter().map_err(|_| QueueError::Disconnected)?;
      self.waiter = Some(waiter);
    }
    // SAFETY: waiter is guaranteed to be Some after the above check.
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
      match this.state.poll() {
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
