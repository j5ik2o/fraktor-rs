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
