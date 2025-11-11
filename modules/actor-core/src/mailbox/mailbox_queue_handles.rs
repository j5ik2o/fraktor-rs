//! Handles for interacting with queue producers/consumers.

use core::cmp;

use cellactor_utils_core_rs::{
  collections::queue::{
    MpscQueue, QueueError, SyncMpscConsumer, SyncMpscProducer, VecRingStorage,
    backend::{OfferOutcome, OverflowPolicy, VecRingBackend},
  },
  runtime_toolbox::SyncMutexFamily,
  sync::ArcShared,
};

use super::{
  QueueMutex, mailbox_queue_offer_future::QueueOfferFuture, mailbox_queue_poll_future::QueuePollFuture,
  mailbox_queue_state::QueueState,
};
use crate::{
  RuntimeToolbox,
  mailbox::{capacity::MailboxCapacity, overflow_strategy::MailboxOverflowStrategy, policy::MailboxPolicy},
};

const DEFAULT_QUEUE_CAPACITY: usize = 16;
type QueueProducer<T, TB> = SyncMpscProducer<T, VecRingBackend<T>, QueueMutex<T, TB>>;
type QueueConsumer<T, TB> = SyncMpscConsumer<T, VecRingBackend<T>, QueueMutex<T, TB>>;

/// Internal handles wrapping queue producers/consumers.
pub struct QueueHandles<T, TB: RuntimeToolbox>
where
  T: Send + 'static, {
  pub(super) state:     ArcShared<QueueState<T, TB>>,
  pub(super) _producer: QueueProducer<T, TB>,
  #[allow(dead_code)]
  pub(super) consumer:  QueueConsumer<T, TB>,
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
    let storage = VecRingStorage::with_capacity(capacity);
    let backend = VecRingBackend::new_with_storage(storage, overflow);
    let mutex = <TB::MutexFamily as SyncMutexFamily>::create(backend);
    let shared = ArcShared::new(mutex);
    let state = ArcShared::new(QueueState::new(shared.clone()));
    let queue: MpscQueue<_, VecRingBackend<T>, _> = MpscQueue::new_mpsc(shared);
    let (producer, consumer) = queue.into_mpsc_pair();
    Self { state, _producer: producer, consumer }
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
