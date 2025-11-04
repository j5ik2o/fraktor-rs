use super::*;

#[test]
fn offer_and_poll_roundtrip() {
  let storage = VecRingStorage::with_capacity(4);
  let mut backend = VecRingBackend::new_with_storage(storage, OverflowPolicy::Block);

  assert_eq!(backend.offer(1).unwrap(), OfferOutcome::Enqueued);
  assert_eq!(backend.offer(2).unwrap(), OfferOutcome::Enqueued);
  assert_eq!(backend.len(), 2);
  assert_eq!(backend.poll().unwrap(), 1);
  assert_eq!(backend.poll().unwrap(), 2);
  assert!(matches!(backend.poll(), Err(QueueError::Empty)));
}

#[test]
fn drop_oldest_replaces_head() {
  let storage = VecRingStorage::with_capacity(2);
  let mut backend = VecRingBackend::new_with_storage(storage, OverflowPolicy::DropOldest);

  backend.offer(10).unwrap();
  backend.offer(20).unwrap();
  let outcome = backend.offer(30).unwrap();
  assert_eq!(outcome, OfferOutcome::DroppedOldest { count: 1 });
  assert_eq!(backend.poll().unwrap(), 20);
  assert_eq!(backend.poll().unwrap(), 30);
}

#[test]
fn drop_newest_discards_new_item() {
  let storage = VecRingStorage::with_capacity(1);
  let mut backend = VecRingBackend::new_with_storage(storage, OverflowPolicy::DropNewest);

  backend.offer(1).unwrap();
  let outcome = backend.offer(2).unwrap();
  assert_eq!(outcome, OfferOutcome::DroppedNewest { count: 1 });
  assert_eq!(backend.poll().unwrap(), 1);
  assert!(matches!(backend.poll(), Err(QueueError::Empty)));
}

#[test]
fn grow_policy_increases_capacity() {
  let storage = VecRingStorage::with_capacity(1);
  let mut backend = VecRingBackend::new_with_storage(storage, OverflowPolicy::Grow);

  backend.offer(1).unwrap();
  let outcome = backend.offer(2).unwrap();
  assert_eq!(outcome, OfferOutcome::GrewTo { capacity: 2 });
  assert_eq!(backend.capacity(), 2);
}

#[test]
fn closed_backend_rejects_offer_and_poll() {
  let storage = VecRingStorage::with_capacity(1);
  let mut backend = VecRingBackend::new_with_storage(storage, OverflowPolicy::Block);

  backend.offer(1).unwrap();
  backend.close();
  assert!(matches!(backend.offer(2), Err(QueueError::Closed(value)) if value == 2));
  assert_eq!(backend.poll().unwrap(), 1);
  assert!(matches!(backend.poll(), Err(QueueError::Disconnected)));
}

#[test]
fn sync_queue_backend_new() {
  let storage = VecRingStorage::with_capacity(5);
  let backend = VecRingBackend::new(storage, OverflowPolicy::Block);
  assert_eq!(backend.capacity(), 5);
  assert_eq!(backend.overflow_policy(), OverflowPolicy::Block);
  assert!(!backend.is_closed());
}

#[test]
fn overflow_policy_returns_correct_policy() {
  let storage = VecRingStorage::with_capacity(1);
  let backend1 = VecRingBackend::new_with_storage(storage, OverflowPolicy::DropNewest);
  assert_eq!(backend1.overflow_policy(), OverflowPolicy::DropNewest);

  let storage = VecRingStorage::with_capacity(1);
  let backend2 = VecRingBackend::new_with_storage(storage, OverflowPolicy::DropOldest);
  assert_eq!(backend2.overflow_policy(), OverflowPolicy::DropOldest);

  let storage = VecRingStorage::with_capacity(1);
  let backend3 = VecRingBackend::new_with_storage(storage, OverflowPolicy::Grow);
  assert_eq!(backend3.overflow_policy(), OverflowPolicy::Grow);
}

#[test]
fn is_closed_returns_false_initially() {
  let storage = VecRingStorage::with_capacity(1);
  let backend = VecRingBackend::new_with_storage(storage, OverflowPolicy::Block);
  assert!(!backend.is_closed());
}

#[test]
fn is_closed_returns_true_after_close() {
  let storage = VecRingStorage::with_capacity(1);
  let mut backend = VecRingBackend::new_with_storage(storage, OverflowPolicy::Block);
  backend.close();
  assert!(backend.is_closed());
}

#[test]
fn capacity_returns_storage_capacity() {
  let storage = VecRingStorage::with_capacity(10);
  let backend = VecRingBackend::new_with_storage(storage, OverflowPolicy::Block);
  assert_eq!(backend.capacity(), 10);
}

#[test]
fn block_policy_returns_full_error_when_full() {
  let storage = VecRingStorage::with_capacity(1);
  let mut backend = VecRingBackend::new_with_storage(storage, OverflowPolicy::Block);

  backend.offer(1).unwrap();
  assert!(matches!(backend.offer(2), Err(QueueError::Full(value)) if value == 2));
}

#[test]
fn empty_queue_returns_empty_error() {
  let storage = VecRingStorage::with_capacity(5);
  let mut backend = VecRingBackend::new_with_storage(storage, OverflowPolicy::Block);
  assert!(matches!(backend.poll(), Err(QueueError::Empty)));
}

#[test]
fn grow_policy_doubles_capacity_when_needed() {
  let storage = VecRingStorage::with_capacity(2);
  let mut backend = VecRingBackend::new_with_storage(storage, OverflowPolicy::Grow);

  backend.offer(1).unwrap();
  backend.offer(2).unwrap();
  // ??offer????2?????2 -> 4?
  let outcome = backend.offer(3).unwrap();
  assert_eq!(outcome, OfferOutcome::GrewTo { capacity: 4 });
  assert_eq!(backend.capacity(), 4);
}
