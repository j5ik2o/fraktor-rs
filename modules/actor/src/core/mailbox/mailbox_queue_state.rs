//! Shared queue state coordinating producers and consumers.

use core::sync::atomic::{AtomicUsize, Ordering};

use fraktor_utils_rs::core::{
  collections::{
    queue::{QueueError, backend::OfferOutcome},
    wait::{WaitError, WaitQueue, WaitShared},
  },
  runtime_toolbox::RuntimeToolbox,
};

use super::UserQueueShared;

/// Maintains shared queue state and wait queues for asynchronous offers/polls.
pub struct QueueState<T, TB: RuntimeToolbox>
where
  T: Send + 'static, {
  pub(crate) queue:            UserQueueShared<T, TB>,
  pub(crate) producer_waiters: WaitQueue<QueueError<T>, TB>,
  pub(crate) consumer_waiters: WaitQueue<QueueError<T>, TB>,
  pub(crate) size:             AtomicUsize,
}

impl<T, TB: RuntimeToolbox + 'static> QueueState<T, TB>
where
  T: Send + 'static,
{
  /// Creates a new queue state wrapper.
  #[must_use]
  pub fn new(queue: UserQueueShared<T, TB>) -> Self {
    Self { queue, producer_waiters: WaitQueue::new(), consumer_waiters: WaitQueue::new(), size: AtomicUsize::new(0) }
  }

  /// Attempts to offer a message into the queue.
  pub(crate) fn offer(&mut self, message: T) -> Result<OfferOutcome, QueueError<T>> {
    let result = self.queue.offer(message);

    if result.is_ok() {
      self.size.fetch_add(1, Ordering::Release);
      self.notify_consumer_waiter();
    }

    result
  }

  /// Attempts to poll a message from the queue.
  pub(crate) fn poll(&mut self) -> Result<T, QueueError<T>> {
    let result = self.queue.poll();

    if result.is_ok() {
      self.size.fetch_sub(1, Ordering::Release);
      self.notify_producer_waiter();
    }

    result
  }

  pub(crate) fn register_producer_waiter(&mut self) -> Result<WaitShared<QueueError<T>, TB>, WaitError> {
    self.producer_waiters.register()
  }

  pub(crate) fn register_consumer_waiter(&mut self) -> Result<WaitShared<QueueError<T>, TB>, WaitError> {
    self.consumer_waiters.register()
  }

  fn notify_producer_waiter(&mut self) {
    let _ = self.producer_waiters.notify_success();
  }

  fn notify_consumer_waiter(&mut self) {
    let _ = self.consumer_waiters.notify_success();
  }

  /// Returns the current queue size without acquiring a lock.
  #[must_use]
  pub(crate) fn len(&self) -> usize {
    self.size.load(Ordering::Acquire)
  }
}
