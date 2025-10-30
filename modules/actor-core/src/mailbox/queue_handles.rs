use core::cmp;

use cellactor_utils_core_rs::{
  collections::queue::{
    MpscQueue, QueueError, SyncMpscConsumer, SyncMpscProducer, VecRingStorage,
    backend::{OfferOutcome, OverflowPolicy, VecRingBackend},
  },
  sync::{ArcShared, sync_mutex_like::SpinSyncMutex},
};

use super::{queue_offer_future::QueueOfferFuture, queue_poll_future::QueuePollFuture, queue_state::QueueState};
use crate::mailbox_policy::{MailboxCapacity, MailboxOverflowStrategy, MailboxPolicy};

type QueueMutex<T> = SpinSyncMutex<VecRingBackend<T>>;
type QueueProducer<T> = SyncMpscProducer<T, VecRingBackend<T>, QueueMutex<T>>;
type QueueConsumer<T> = SyncMpscConsumer<T, VecRingBackend<T>, QueueMutex<T>>;

const DEFAULT_QUEUE_CAPACITY: usize = 16;
const SYSTEM_QUEUE_CAPACITY: usize = 8;

pub(super) struct QueueHandles<T> {
  pub(super) state:     ArcShared<QueueState<T>>,
  pub(super) _producer: QueueProducer<T>,
  pub(super) consumer:  QueueConsumer<T>,
}

impl<T> QueueHandles<T> {
  pub(super) fn new_user(policy: &MailboxPolicy) -> Self {
    let (capacity, overflow) = match policy.capacity() {
      | MailboxCapacity::Bounded { capacity } => (cmp::max(1, capacity.get()), map_overflow(policy.overflow())),
      | MailboxCapacity::Unbounded => (DEFAULT_QUEUE_CAPACITY, OverflowPolicy::Grow),
    };
    QueueHandles::new_with(capacity, overflow)
  }

  pub(super) fn new_system() -> Self {
    QueueHandles::new_with(SYSTEM_QUEUE_CAPACITY, OverflowPolicy::Grow)
  }

  fn new_with(capacity: usize, overflow: OverflowPolicy) -> Self {
    let storage = VecRingStorage::with_capacity(capacity);
    let backend = VecRingBackend::new_with_storage(storage, overflow);
    let mutex = SpinSyncMutex::new(backend);
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

  pub(super) fn offer_blocking(&self, message: T) -> QueueOfferFuture<T> {
    QueueOfferFuture::new(self.state.clone(), message)
  }

  pub(super) fn poll_blocking(&self) -> QueuePollFuture<T> {
    QueuePollFuture::new(self.state.clone())
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
