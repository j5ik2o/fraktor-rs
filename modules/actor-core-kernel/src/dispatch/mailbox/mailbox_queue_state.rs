//! Shared queue state coordinating producers and consumers.

use core::sync::atomic::{AtomicUsize, Ordering};

use fraktor_utils_core_rs::{
  collections::{
    queue::{OfferOutcome, QueueError, SyncQueue, backend::VecDequeBackend},
    wait::{WaitError, WaitQueue, WaitShared},
  },
  sync::{DefaultMutex, SharedLock},
};

use super::UserQueueShared;

/// Maintains shared queue state and wait queues for asynchronous offers/polls.
pub(crate) struct QueueState<T>
where
  T: Send + 'static, {
  pub(crate) queue:            UserQueueShared<T>,
  pub(crate) producer_waiters: WaitQueue<QueueError<T>>,
  pub(crate) consumer_waiters: WaitQueue<QueueError<T>>,
  pub(crate) size:             AtomicUsize,
}

impl<T> QueueState<T>
where
  T: Send + 'static,
{
  /// Creates a new queue state wrapper.
  #[must_use]
  pub(crate) fn new(queue: UserQueueShared<T>) -> Self {
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

  pub(crate) fn register_consumer_waiter(&mut self) -> Result<WaitShared<QueueError<T>>, WaitError> {
    self.consumer_waiters.register()
  }

  fn notify_producer_waiter(&mut self) {
    // must-ignore: notify_success の bool (待機者有無) は通知者側で分岐に使わないため破棄する。
    let _ = self.producer_waiters.notify_success();
  }

  fn notify_consumer_waiter(&mut self) {
    // must-ignore: notify_success の bool (待機者有無) は通知者側で分岐に使わないため破棄する。
    let _ = self.consumer_waiters.notify_success();
  }

  /// Returns the current queue size without acquiring a lock.
  #[must_use]
  pub(crate) fn len(&self) -> usize {
    self.size.load(Ordering::Acquire)
  }
}

pub(crate) fn queue_state_shared<T>(queue: SyncQueue<T, VecDequeBackend<T>>) -> SharedLock<QueueState<T>>
where
  T: Send + 'static, {
  let queue = UserQueueShared::new_with_builtin_lock(queue);
  SharedLock::new_with_driver::<DefaultMutex<_>>(QueueState::new(queue))
}
