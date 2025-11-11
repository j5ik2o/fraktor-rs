//! Shared queue state coordinating producers and consumers.

use core::sync::atomic::{AtomicUsize, Ordering};

use cellactor_utils_core_rs::{
  collections::{
    queue::{QueueError, SyncQueueBackend, backend::OfferOutcome},
    wait::{WaitQueue, WaitShared},
  },
  sync::{ArcShared, sync_mutex_like::SyncMutexLike},
};
use spin::Mutex;

use super::QueueMutex;
use crate::RuntimeToolbox;

/// Maintains shared queue state and wait queues for asynchronous offers/polls.
pub struct QueueState<T, TB: RuntimeToolbox>
where
  T: Send + 'static, {
  pub(super) shared:           ArcShared<QueueMutex<T, TB>>,
  pub(super) producer_waiters: Mutex<WaitQueue<QueueError<T>>>,
  pub(super) consumer_waiters: Mutex<WaitQueue<QueueError<T>>>,
  pub(super) size:             AtomicUsize,
}

impl<T, TB: RuntimeToolbox> QueueState<T, TB>
where
  T: Send + 'static,
{
  /// Creates a new queue state wrapper.
  #[must_use]
  pub const fn new(shared: ArcShared<QueueMutex<T, TB>>) -> Self {
    Self {
      shared,
      producer_waiters: Mutex::new(WaitQueue::new()),
      consumer_waiters: Mutex::new(WaitQueue::new()),
      size: AtomicUsize::new(0),
    }
  }

  /// Attempts to offer a message into the queue.
  pub(super) fn offer(&self, message: T) -> Result<OfferOutcome, QueueError<T>> {
    let result = {
      let mut guard = self.shared.lock();
      guard.offer(message)
    };

    if result.is_ok() {
      self.size.fetch_add(1, Ordering::Release);
      self.notify_consumer_waiter();
    }

    result
  }

  /// Attempts to poll a message from the queue.
  pub(super) fn poll(&self) -> Result<T, QueueError<T>> {
    let result = {
      let mut guard = self.shared.lock();
      guard.poll()
    };

    if result.is_ok() {
      self.size.fetch_sub(1, Ordering::Release);
      self.notify_producer_waiter();
    }

    result
  }

  pub(super) fn register_producer_waiter(&self) -> WaitShared<QueueError<T>> {
    self.producer_waiters.lock().register()
  }

  pub(super) fn register_consumer_waiter(&self) -> WaitShared<QueueError<T>> {
    self.consumer_waiters.lock().register()
  }

  fn notify_producer_waiter(&self) {
    let _ = self.producer_waiters.lock().notify_success();
  }

  fn notify_consumer_waiter(&self) {
    let _ = self.consumer_waiters.lock().notify_success();
  }

  /// Returns the current queue size without acquiring a lock.
  #[must_use]
  pub(super) fn len(&self) -> usize {
    self.size.load(Ordering::Acquire)
  }
}
