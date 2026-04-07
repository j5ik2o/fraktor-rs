extern crate alloc;

use super::{QueueError, SyncQueue, SyncQueueShared};
use crate::core::sync::{ArcShared, SpinSyncMutex};

mod storage_config {
  /// Storage configuration used by the test backends.
  #[derive(Clone, Copy, Default)]
  pub(crate) struct QueueConfig {
    capacity: usize,
  }

  impl QueueConfig {
    /// Creates a new configuration with the specified capacity.
    pub(crate) fn new(capacity: usize) -> Self {
      Self { capacity }
    }

    /// Returns the configured capacity.
    #[must_use]
    pub(crate) const fn capacity(self) -> usize {
      self.capacity
    }
  }
}

use storage_config::QueueConfig;

mod fifo_backend {
  use alloc::collections::VecDeque;

  use super::QueueConfig;
  use crate::core::collections::queue::{
    QueueError,
    backend::{SyncQueueBackend, SyncQueueBackendInternal},
    offer_outcome::OfferOutcome,
    overflow_policy::OverflowPolicy,
  };

  /// Simple FIFO backend used for unit tests.
  pub(crate) struct FifoBackend<T> {
    buffer:   VecDeque<T>,
    capacity: usize,
    policy:   OverflowPolicy,
    closed:   bool,
  }

  impl<T> FifoBackend<T> {
    /// Creates a backend with the provided capacity and overflow policy.
    pub(crate) fn new(storage: QueueConfig, policy: OverflowPolicy) -> Self {
      Self { buffer: VecDeque::new(), capacity: storage.capacity(), policy, closed: false }
    }
  }

  impl<T> SyncQueueBackend<T> for FifoBackend<T> {}

  impl<T> SyncQueueBackendInternal<T> for FifoBackend<T> {
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

use crate::core::collections::queue::{
  OfferOutcome, OverflowPolicy,
  backend::VecDequeBackend,
  capabilities::{QueueCapability, QueueCapabilityRegistry, QueueCapabilitySet},
};

#[test]
fn offer_and_poll_fifo_queue() {
  let backend = FifoBackend::new(QueueConfig::new(2), OverflowPolicy::DropOldest);
  let sync_queue = SyncQueue::new(backend);
  let shared = ArcShared::new(SpinSyncMutex::new(sync_queue));
  let queue: SyncQueueShared<_, _> = SyncQueueShared::new(shared);

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
fn shared_error_mapping_matches_spec() {
  use crate::core::sync::SharedError;

  assert_eq!(QueueError::<()>::from(SharedError::Poisoned), QueueError::Disconnected);
  assert_eq!(QueueError::<()>::from(SharedError::BorrowConflict), QueueError::WouldBlock);
  assert_eq!(QueueError::<()>::from(SharedError::InterruptContext), QueueError::WouldBlock);
}

#[test]
fn vec_ring_backend_provides_fifo_behavior() {
  let backend = VecDequeBackend::with_capacity(3, OverflowPolicy::DropOldest);
  let sync_queue = SyncQueue::new(backend);
  let shared = ArcShared::new(SpinSyncMutex::new(sync_queue));
  let queue: SyncQueueShared<_, _> = SyncQueueShared::new(shared);

  assert_eq!(queue.offer(1).unwrap(), OfferOutcome::Enqueued);
  assert_eq!(queue.offer(2).unwrap(), OfferOutcome::Enqueued);
  assert_eq!(queue.offer(3).unwrap(), OfferOutcome::Enqueued);

  let outcome = queue.offer(4).unwrap();
  assert_eq!(outcome, OfferOutcome::DroppedOldest { count: 1 });
  assert_eq!(queue.poll().unwrap(), 2);
  assert_eq!(queue.poll().unwrap(), 3);
  assert_eq!(queue.poll().unwrap(), 4);
}

#[test]
fn queue_capability_registry_reports_missing_capability() {
  let registry = QueueCapabilityRegistry::new(QueueCapabilitySet::default().with_deque(false));
  let err = registry.ensure(QueueCapability::Deque).expect_err("deque capability should be missing");
  assert_eq!(err.missing(), QueueCapability::Deque);
}
