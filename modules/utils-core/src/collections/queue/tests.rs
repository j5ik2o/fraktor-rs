extern crate alloc;

use super::SyncQueue;
use crate::{
  collections::queue::QueueError,
  sync::{ArcShared, sync_mutex_like::SpinSyncMutex},
};

mod storage_config {
  use crate::collections::queue::QueueStorage;

  /// Storage configuration used by the test backends.
  #[derive(Clone, Copy, Default)]
  pub struct QueueConfig {
    capacity: usize,
  }

  impl QueueConfig {
    /// Creates a new configuration with the specified capacity.
    pub fn new(capacity: usize) -> Self {
      Self { capacity }
    }

    /// Returns the configured capacity.
    #[must_use]
    pub const fn capacity(self) -> usize {
      self.capacity
    }
  }

  impl<T> QueueStorage<T> for QueueConfig {
    fn capacity(&self) -> usize {
      self.capacity
    }

    unsafe fn read_unchecked(&self, _idx: usize) -> *const T {
      core::ptr::null()
    }

    unsafe fn write_unchecked(&mut self, _idx: usize, val: T) {
      core::mem::drop(val);
      panic!("write_unchecked is not supported in test storage");
    }
  }
}

use storage_config::QueueConfig;

mod fifo_backend {
  use alloc::collections::VecDeque;

  use super::QueueConfig;
  use crate::collections::queue::{
    QueueError,
    backend::{OfferOutcome, OverflowPolicy, SyncQueueBackend},
  };

  /// Simple FIFO backend used for unit tests.
  pub struct FifoBackend<T> {
    buffer:   VecDeque<T>,
    capacity: usize,
    policy:   OverflowPolicy,
    closed:   bool,
  }

  impl<T> FifoBackend<T> {
    /// Creates a backend with the provided capacity and overflow policy.
    pub fn new(storage: QueueConfig, policy: OverflowPolicy) -> Self {
      Self { buffer: VecDeque::new(), capacity: storage.capacity(), policy, closed: false }
    }
  }

  impl<T> SyncQueueBackend<T> for FifoBackend<T> {
    type Storage = QueueConfig;

    fn new(storage: Self::Storage, policy: OverflowPolicy) -> Self {
      FifoBackend::new(storage, policy)
    }

    fn offer(&mut self, item: T) -> Result<OfferOutcome, QueueError<T>> {
      if self.closed {
        return Err(QueueError::Closed(item));
      }
      if self.buffer.len() < self.capacity {
        self.buffer.push_back(item);
        return Ok(OfferOutcome::Enqueued);
      }
      match self.policy {
        | OverflowPolicy::DropNewest => Ok(OfferOutcome::DroppedNewest { count: 1 }),
        | OverflowPolicy::DropOldest => {
          let _ = self.buffer.pop_front();
          self.buffer.push_back(item);
          Ok(OfferOutcome::DroppedOldest { count: 1 })
        },
        | OverflowPolicy::Block => Err(QueueError::Full(item)),
        | OverflowPolicy::Grow => {
          self.capacity += 1;
          self.buffer.push_back(item);
          Ok(OfferOutcome::GrewTo { capacity: self.capacity })
        },
      }
    }

    fn poll(&mut self) -> Result<T, QueueError<T>> {
      match self.buffer.pop_front() {
        | Some(item) => Ok(item),
        | None => {
          if self.closed {
            Err(QueueError::Disconnected)
          } else {
            Err(QueueError::Empty)
          }
        },
      }
    }

    fn len(&self) -> usize {
      self.buffer.len()
    }

    fn capacity(&self) -> usize {
      self.capacity
    }

    fn overflow_policy(&self) -> OverflowPolicy {
      self.policy
    }

    fn is_closed(&self) -> bool {
      self.closed
    }

    fn close(&mut self) {
      self.closed = true;
    }
  }
}

use fifo_backend::FifoBackend;

mod mpsc_key_capability_assertion {
  use crate::collections::queue::{
    capabilities::{MultiProducer, SingleConsumer},
    type_keys::MpscKey,
  };

  /// Ensures capability traits are implemented for MpscKey.
  pub fn assert_capabilities() {
    fn require_capabilities<K: MultiProducer + SingleConsumer>() {}
    require_capabilities::<MpscKey>();
  }
}

mod priority_message {
  use crate::PriorityMessage;

  /// Priority-aware message used in priority queue tests.
  #[derive(Clone, Debug, PartialEq, Eq)]
  pub struct TestPriorityMessage {
    value:    i32,
    priority: Option<i8>,
  }

  impl TestPriorityMessage {
    /// Creates a new message with the specified value and priority.
    #[must_use]
    pub const fn new(value: i32, priority: Option<i8>) -> Self {
      Self { value, priority }
    }

    /// Returns the stored payload.
    #[must_use]
    pub const fn value(&self) -> i32 {
      self.value
    }
  }

  impl PriorityMessage for TestPriorityMessage {
    fn get_priority(&self) -> Option<i8> {
      self.priority
    }
  }
}

use priority_message::TestPriorityMessage;

use crate::{
  collections::queue::{
    VecRingStorage,
    backend::{OfferOutcome, OverflowPolicy, VecRingBackend, sync_priority_backend::BinaryHeapPriorityBackend},
    capabilities::{SingleConsumer, SingleProducer, SupportsPeek},
    type_keys::{FifoKey, MpscKey, PriorityKey, SpscKey},
  },
  sync::SharedError,
};

#[test]
fn offer_and_poll_fifo_queue() {
  mpsc_key_capability_assertion::assert_capabilities();

  let backend = FifoBackend::new(QueueConfig::new(2), OverflowPolicy::DropOldest);
  let shared = ArcShared::new(SpinSyncMutex::new(backend));
  let queue: SyncQueue<_, FifoKey, _, _> = SyncQueue::new(shared);

  assert_eq!(queue.offer(1).unwrap(), OfferOutcome::Enqueued);
  assert_eq!(queue.offer(2).unwrap(), OfferOutcome::Enqueued);

  let outcome = queue.offer(3).unwrap();
  assert_eq!(outcome, OfferOutcome::DroppedOldest { count: 1 });
  assert_eq!(queue.len(), 2);
  assert_eq!(queue.poll().unwrap(), 2);
  assert_eq!(queue.poll().unwrap(), 3);
  assert!(matches!(queue.poll(), Err(QueueError::Empty)));
}

#[test]
fn block_policy_reports_full() {
  let backend = FifoBackend::new(QueueConfig::new(1), OverflowPolicy::Block);
  let shared = ArcShared::new(SpinSyncMutex::new(backend));
  let queue: SyncQueue<_, SpscKey, _, _> = SyncQueue::new(shared);

  assert_eq!(queue.offer(10).unwrap(), OfferOutcome::Enqueued);
  let err = queue.offer(20).unwrap_err();
  assert!(matches!(err, QueueError::Full(value) if value == 20));
}

#[test]
fn grow_policy_increases_capacity() {
  let backend = FifoBackend::new(QueueConfig::new(1), OverflowPolicy::Grow);
  let shared = ArcShared::new(SpinSyncMutex::new(backend));
  let queue: SyncQueue<_, MpscKey, _, _> = SyncQueue::new(shared.clone());

  assert_eq!(queue.offer(1).unwrap(), OfferOutcome::Enqueued);
  let outcome = queue.offer(2).unwrap();
  assert_eq!(outcome, OfferOutcome::GrewTo { capacity: 2 });
  assert_eq!(queue.capacity(), 2);

  queue.close().unwrap();
  assert!(matches!(queue.offer(3), Err(QueueError::Closed(value)) if value == 3));
  assert_eq!(queue.poll().unwrap(), 1);
  assert_eq!(queue.poll().unwrap(), 2);
  assert!(matches!(queue.poll(), Err(QueueError::Disconnected)));
}

#[test]
fn priority_queue_supports_peek() {
  fn assert_priority_capabilities<K: SingleProducer + SingleConsumer + SupportsPeek>() {}
  assert_priority_capabilities::<PriorityKey>();

  let backend = BinaryHeapPriorityBackend::new_with_capacity(4, OverflowPolicy::DropOldest);
  let shared = ArcShared::new(SpinSyncMutex::new(backend));
  let queue: SyncQueue<TestPriorityMessage, PriorityKey, _, _> = SyncQueue::new(shared);

  assert_eq!(queue.offer(TestPriorityMessage::new(5, Some(2))).unwrap(), OfferOutcome::Enqueued);
  assert_eq!(queue.offer(TestPriorityMessage::new(2, Some(0))).unwrap(), OfferOutcome::Enqueued);
  assert_eq!(queue.offer(TestPriorityMessage::new(7, Some(5))).unwrap(), OfferOutcome::Enqueued);

  let peeked = queue.peek_min().unwrap().map(|msg| msg.value());
  assert_eq!(peeked, Some(2));
  assert_eq!(queue.poll().unwrap().value(), 7);
  assert_eq!(queue.peek_min().unwrap().map(|msg| msg.value()), Some(2));
}

#[test]
fn shared_error_mapping_matches_spec() {
  assert_eq!(QueueError::<()>::from(SharedError::Poisoned), QueueError::Disconnected);
  assert_eq!(QueueError::<()>::from(SharedError::BorrowConflict), QueueError::WouldBlock);
  assert_eq!(QueueError::<()>::from(SharedError::InterruptContext), QueueError::WouldBlock);
}

#[test]
fn mpsc_pair_supports_multiple_producers() {
  let storage = VecRingStorage::with_capacity(8);
  let backend = VecRingBackend::new_with_storage(storage, OverflowPolicy::DropOldest);
  let shared = ArcShared::new(SpinSyncMutex::new(backend));
  let queue: SyncQueue<_, MpscKey, _, _> = SyncQueue::new(shared);

  let (producer, consumer) = queue.into_mpsc_pair();
  let another = producer.clone();

  producer.offer(1).unwrap();
  another.offer(2).unwrap();
  producer.offer(3).unwrap();

  let mut collected = [consumer.poll().unwrap(), consumer.poll().unwrap(), consumer.poll().unwrap()];
  collected.sort();
  assert_eq!(collected, [1, 2, 3]);
}

#[test]
fn spsc_pair_provides_split_access() {
  let storage = VecRingStorage::with_capacity(4);
  let backend = VecRingBackend::new_with_storage(storage, OverflowPolicy::Block);
  let shared = ArcShared::new(SpinSyncMutex::new(backend));
  let queue: SyncQueue<_, SpscKey, _, _> = SyncQueue::new(shared);

  let (producer, consumer) = queue.into_spsc_pair();
  producer.offer(10).unwrap();
  producer.offer(20).unwrap();

  assert_eq!(consumer.poll().unwrap(), 10);
  assert_eq!(consumer.poll().unwrap(), 20);
}

#[test]
fn vec_ring_backend_provides_fifo_behavior() {
  let storage = VecRingStorage::with_capacity(3);
  let backend = VecRingBackend::new_with_storage(storage, OverflowPolicy::DropOldest);
  let shared = ArcShared::new(SpinSyncMutex::new(backend));
  let queue: SyncQueue<_, FifoKey, _, _> = SyncQueue::new(shared);

  assert_eq!(queue.offer(1).unwrap(), OfferOutcome::Enqueued);
  assert_eq!(queue.offer(2).unwrap(), OfferOutcome::Enqueued);
  assert_eq!(queue.offer(3).unwrap(), OfferOutcome::Enqueued);

  let outcome = queue.offer(4).unwrap();
  assert_eq!(outcome, OfferOutcome::DroppedOldest { count: 1 });
  assert_eq!(queue.poll().unwrap(), 2);
  assert_eq!(queue.poll().unwrap(), 3);
  assert_eq!(queue.poll().unwrap(), 4);
}
