//! Handles for interacting with queue producers/consumers.

use core::cmp;

use fraktor_utils_core_rs::core::{
  collections::queue::{OfferOutcome, OverflowPolicy, QueueError, SyncQueue, backend::VecDequeBackend},
  sync::{ArcShared, RuntimeMutex, SpinSyncMutex},
};

use super::{UserQueueShared, mailbox_queue_state::QueueState};
use crate::core::kernel::dispatch::mailbox::{
  capacity::MailboxCapacity, overflow_strategy::MailboxOverflowStrategy, policy::MailboxPolicy,
};

const DEFAULT_QUEUE_CAPACITY: usize = 16;

/// Internal handles wrapping queue producers/consumers.
pub(crate) struct QueueStateHandle<T>
where
  T: Send + 'static, {
  pub(crate) state: ArcShared<RuntimeMutex<QueueState<T>>>,
}

impl<T> Clone for QueueStateHandle<T>
where
  T: Send + 'static,
{
  fn clone(&self) -> Self {
    Self { state: self.state.clone() }
  }
}

impl<T> QueueStateHandle<T>
where
  T: Send + 'static,
{
  pub(crate) fn new_user(policy: &MailboxPolicy) -> Self {
    let (capacity, overflow) = match policy.capacity() {
      | MailboxCapacity::Bounded { capacity } => (cmp::max(1, capacity.get()), map_overflow(policy.overflow())),
      | MailboxCapacity::Unbounded => (DEFAULT_QUEUE_CAPACITY, OverflowPolicy::Grow),
    };
    Self::new_with(capacity, overflow)
  }

  fn new_with(capacity: usize, overflow: OverflowPolicy) -> Self {
    let backend = VecDequeBackend::with_capacity(capacity, overflow);
    let sync_queue = SyncQueue::new(backend);
    let mutex = SpinSyncMutex::new(sync_queue);
    let queue = UserQueueShared::<T>::new(ArcShared::new(mutex));
    let state_mutex = RuntimeMutex::new(QueueState::new(queue));
    let state = ArcShared::new(state_mutex);
    Self { state }
  }

  pub(crate) fn offer(&self, message: T) -> Result<OfferOutcome, QueueError<T>> {
    let mut state = self.state.lock();
    state.offer(message)
  }

  pub(crate) fn offer_if_room(&self, message: T, capacity: usize) -> Result<OfferOutcome, QueueError<T>> {
    let mut state = self.state.lock();
    if state.len() >= capacity {
      return Err(QueueError::Full(message));
    }
    state.offer(message)
  }

  pub(crate) fn drop_oldest_and_offer(&self, message: T, capacity: usize) -> Result<OfferOutcome, QueueError<T>> {
    let mut state = self.state.lock();
    if state.len() >= capacity {
      // Intentionally discard the oldest element to make room for the new message.
      let _oldest = state.poll();
    }
    state.offer(message)
  }

  pub(crate) fn poll(&self) -> Result<T, QueueError<T>> {
    let mut state = self.state.lock();
    state.poll()
  }

  /// Returns the current queue size without acquiring a lock.
  #[must_use]
  pub(crate) fn len(&self) -> usize {
    self.state.lock().len()
  }
}

const fn map_overflow(strategy: MailboxOverflowStrategy) -> OverflowPolicy {
  match strategy {
    | MailboxOverflowStrategy::DropNewest => OverflowPolicy::DropNewest,
    | MailboxOverflowStrategy::DropOldest => OverflowPolicy::DropOldest,
    | MailboxOverflowStrategy::Grow => OverflowPolicy::Grow,
  }
}
