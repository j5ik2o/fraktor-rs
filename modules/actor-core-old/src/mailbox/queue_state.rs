use cellactor_utils_core_rs::{
  collections::{
    queue::{QueueError, SyncQueueBackend, backend::OfferOutcome},
    wait::{WaitHandle, WaitQueue},
  },
  sync::ArcShared,
};
use spin::Mutex;

use super::QueueMutex;

pub(super) struct QueueState<T> {
  pub(super) shared:           ArcShared<QueueMutex<T>>,
  pub(super) producer_waiters: Mutex<WaitQueue<QueueError<T>>>,
  pub(super) consumer_waiters: Mutex<WaitQueue<QueueError<T>>>,
}

impl<T> QueueState<T> {
  pub(super) const fn new(shared: ArcShared<QueueMutex<T>>) -> Self {
    Self { shared, producer_waiters: Mutex::new(WaitQueue::new()), consumer_waiters: Mutex::new(WaitQueue::new()) }
  }

  pub(super) fn offer(&self, message: T) -> Result<OfferOutcome, QueueError<T>> {
    let result = {
      let mut guard = self.shared.lock();
      guard.offer(message)
    };

    if result.is_ok() {
      self.notify_consumer_waiter();
    }

    result
  }

  pub(super) fn poll(&self) -> Result<T, QueueError<T>> {
    let result = {
      let mut guard = self.shared.lock();
      guard.poll()
    };

    if result.is_ok() {
      self.notify_producer_waiter();
    }

    result
  }

  pub(super) fn register_producer_waiter(&self) -> WaitHandle<QueueError<T>> {
    self.producer_waiters.lock().register()
  }

  pub(super) fn register_consumer_waiter(&self) -> WaitHandle<QueueError<T>> {
    self.consumer_waiters.lock().register()
  }

  fn notify_producer_waiter(&self) {
    let _ = self.producer_waiters.lock().notify_success();
  }

  fn notify_consumer_waiter(&self) {
    let _ = self.consumer_waiters.lock().notify_success();
  }
}
