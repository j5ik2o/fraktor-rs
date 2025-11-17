//! Handles for interacting with queue producers/consumers.

use core::cmp;

use fraktor_utils_core_rs::core::{
  collections::queue::{
    QueueError, SyncQueue,
    backend::{OfferOutcome, OverflowPolicy, VecDequeBackend},
  },
  sync::{ArcShared, sync_mutex_like::SpinSyncMutex},
};

use super::{
  UserQueueShared, mailbox_queue_offer_future::QueueOfferFuture, mailbox_queue_poll_future::QueuePollFuture,
  mailbox_queue_state::QueueState,
};
use crate::{
  RuntimeToolbox,
  mailbox::{capacity::MailboxCapacity, overflow_strategy::MailboxOverflowStrategy, policy::MailboxPolicy},
};

const DEFAULT_QUEUE_CAPACITY: usize = 16;

/// Internal handles wrapping queue producers/consumers.
pub struct QueueHandles<T, TB: RuntimeToolbox>
where
  T: Send + 'static, {
  pub(super) state: ArcShared<QueueState<T, TB>>,
}

impl<T, TB> QueueHandles<T, TB>
where
  T: Send + 'static,
  TB: RuntimeToolbox + 'static,
{
  pub(super) fn new_user(policy: &MailboxPolicy) -> Self {
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
    let queue = UserQueueShared::new(ArcShared::new(mutex));
    let state = ArcShared::new(QueueState::new(queue));
    Self { state }
  }

  pub(super) fn offer(&self, message: T) -> Result<OfferOutcome, QueueError<T>> {
    self.state.offer(message)
  }

  pub(super) fn poll(&self) -> Result<T, QueueError<T>> {
    self.state.poll()
  }

  pub(super) fn offer_blocking(&self, message: T) -> QueueOfferFuture<T, TB> {
    QueueOfferFuture::new(self.state.clone(), message)
  }

  #[allow(dead_code)]
  pub(super) fn poll_blocking(&self) -> QueuePollFuture<T, TB> {
    QueuePollFuture::new(self.state.clone())
  }

  /// Returns the current queue size without acquiring a lock.
  #[must_use]
  pub(super) fn len(&self) -> usize {
    self.state.len()
  }
}

const fn map_overflow(strategy: MailboxOverflowStrategy) -> OverflowPolicy {
  match strategy {
    | MailboxOverflowStrategy::DropNewest => OverflowPolicy::DropNewest,
    | MailboxOverflowStrategy::DropOldest => OverflowPolicy::DropOldest,
    | MailboxOverflowStrategy::Grow => OverflowPolicy::Grow,
    | MailboxOverflowStrategy::Block => OverflowPolicy::Block,
  }
}
