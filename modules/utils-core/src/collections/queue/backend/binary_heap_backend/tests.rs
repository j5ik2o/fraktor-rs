use crate::collections::queue::{
  OfferOutcome, OverflowPolicy, QueueError,
  backend::{BinaryHeapBackend, sync_queue_backend_internal::SyncQueueBackendInternal},
};

#[test]
fn test_binary_heap_backend_basic() {
  let mut backend = BinaryHeapBackend::with_capacity(3, OverflowPolicy::Block);

  // Offer elements
  assert_eq!(backend.offer(10).unwrap(), OfferOutcome::Enqueued);
  assert_eq!(backend.offer(5).unwrap(), OfferOutcome::Enqueued);
  assert_eq!(backend.offer(15).unwrap(), OfferOutcome::Enqueued);

  assert_eq!(backend.len(), 3);

  // Poll returns maximum element first
  assert_eq!(backend.poll().unwrap(), 15);
  assert_eq!(backend.poll().unwrap(), 10);
  assert_eq!(backend.poll().unwrap(), 5);

  assert_eq!(backend.len(), 0);
}

#[test]
fn test_binary_heap_backend_overflow_block() {
  let mut backend = BinaryHeapBackend::with_capacity(2, OverflowPolicy::Block);

  assert_eq!(backend.offer(1).unwrap(), OfferOutcome::Enqueued);
  assert_eq!(backend.offer(2).unwrap(), OfferOutcome::Enqueued);

  // Should block when full
  assert!(matches!(backend.offer(3), Err(QueueError::Full(3))));
}

#[test]
fn test_binary_heap_backend_overflow_drop_newest() {
  let mut backend = BinaryHeapBackend::with_capacity(2, OverflowPolicy::DropNewest);

  assert_eq!(backend.offer(1).unwrap(), OfferOutcome::Enqueued);
  assert_eq!(backend.offer(2).unwrap(), OfferOutcome::Enqueued);

  // Should drop newest when full
  assert_eq!(backend.offer(3).unwrap(), OfferOutcome::DroppedNewest { count: 1 });

  assert_eq!(backend.len(), 2);
  assert_eq!(backend.poll().unwrap(), 2);
  assert_eq!(backend.poll().unwrap(), 1);
}

#[test]
fn test_binary_heap_backend_overflow_drop_oldest() {
  let mut backend = BinaryHeapBackend::with_capacity(2, OverflowPolicy::DropOldest);

  assert_eq!(backend.offer(1).unwrap(), OfferOutcome::Enqueued);
  assert_eq!(backend.offer(2).unwrap(), OfferOutcome::Enqueued);

  // Should drop maximum (oldest) when full
  assert_eq!(backend.offer(3).unwrap(), OfferOutcome::DroppedOldest { count: 1 });

  assert_eq!(backend.len(), 2);
  assert_eq!(backend.poll().unwrap(), 3);
  assert_eq!(backend.poll().unwrap(), 1);
}

#[test]
fn test_binary_heap_backend_overflow_grow() {
  let mut backend = BinaryHeapBackend::with_capacity(2, OverflowPolicy::Grow);

  assert_eq!(backend.offer(1).unwrap(), OfferOutcome::Enqueued);
  assert_eq!(backend.offer(2).unwrap(), OfferOutcome::Enqueued);

  // Should grow when full
  assert!(matches!(backend.offer(3).unwrap(), OfferOutcome::GrewTo { capacity } if capacity >= 3));

  assert_eq!(backend.len(), 3);
  assert!(backend.capacity() >= 3);
}

#[test]
fn test_binary_heap_backend_close() {
  let mut backend = BinaryHeapBackend::with_capacity(3, OverflowPolicy::Block);

  backend.offer(1).unwrap();
  backend.close();

  assert!(backend.is_closed());
  assert!(matches!(backend.offer(2), Err(QueueError::Closed(2))));
  assert_eq!(backend.poll().unwrap(), 1);
  assert!(matches!(backend.poll(), Err(QueueError::Disconnected)));
}
