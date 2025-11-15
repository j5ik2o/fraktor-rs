use super::*;

#[test]
fn offer_and_poll_roundtrip() {
  let mut backend = VecDequeBackend::with_capacity(4, OverflowPolicy::Block);

  assert_eq!(backend.offer(1).unwrap(), OfferOutcome::Enqueued);
  assert_eq!(backend.offer(2).unwrap(), OfferOutcome::Enqueued);
  assert_eq!(backend.len(), 2);
  assert_eq!(backend.poll().unwrap(), 1);
  assert_eq!(backend.poll().unwrap(), 2);
  assert!(matches!(backend.poll(), Err(QueueError::Empty)));
}

#[test]
fn drop_oldest_replaces_head() {
  let mut backend = VecDequeBackend::with_capacity(2, OverflowPolicy::DropOldest);

  backend.offer(10).unwrap();
  backend.offer(20).unwrap();
  let outcome = backend.offer(30).unwrap();
  assert_eq!(outcome, OfferOutcome::DroppedOldest { count: 1 });
  assert_eq!(backend.poll().unwrap(), 20);
  assert_eq!(backend.poll().unwrap(), 30);
}

#[test]
fn drop_newest_discards_new_item() {
  let mut backend = VecDequeBackend::with_capacity(1, OverflowPolicy::DropNewest);

  backend.offer(1).unwrap();
  let outcome = backend.offer(2).unwrap();
  assert_eq!(outcome, OfferOutcome::DroppedNewest { count: 1 });
  assert_eq!(backend.poll().unwrap(), 1);
  assert!(matches!(backend.poll(), Err(QueueError::Empty)));
}

#[test]
fn grow_policy_increases_capacity() {
  let mut backend = VecDequeBackend::with_capacity(1, OverflowPolicy::Grow);

  backend.offer(1).unwrap();
  let outcome = backend.offer(2).unwrap();
  assert_eq!(outcome, OfferOutcome::GrewTo { capacity: 2 });
  assert_eq!(backend.capacity(), 2);
}

#[test]
fn closed_backend_rejects_offer_and_poll() {
  let mut backend = VecDequeBackend::with_capacity(1, OverflowPolicy::Block);

  backend.offer(1).unwrap();
  backend.close();
  assert!(matches!(backend.offer(2), Err(QueueError::Closed(value)) if value == 2));
  assert_eq!(backend.poll().unwrap(), 1);
  assert!(matches!(backend.poll(), Err(QueueError::Disconnected)));
}

#[test]
fn sync_queue_backend_new() {
  let backend = VecDequeBackend::<i32>::with_capacity(5, OverflowPolicy::Block);
  assert_eq!(backend.capacity(), 5);
  assert_eq!(backend.overflow_policy(), OverflowPolicy::Block);
  assert!(!backend.is_closed());
}

#[test]
fn overflow_policy_returns_correct_policy() {
  let backend1 = VecDequeBackend::<i32>::with_capacity(1, OverflowPolicy::DropNewest);
  assert_eq!(backend1.overflow_policy(), OverflowPolicy::DropNewest);

  let backend2 = VecDequeBackend::<i32>::with_capacity(1, OverflowPolicy::DropOldest);
  assert_eq!(backend2.overflow_policy(), OverflowPolicy::DropOldest);

  let backend3 = VecDequeBackend::<i32>::with_capacity(1, OverflowPolicy::Grow);
  assert_eq!(backend3.overflow_policy(), OverflowPolicy::Grow);
}

#[test]
fn is_closed_returns_false_initially() {
  let backend = VecDequeBackend::<i32>::with_capacity(1, OverflowPolicy::Block);
  assert!(!backend.is_closed());
}

#[test]
fn is_closed_returns_true_after_close() {
  let mut backend = VecDequeBackend::<i32>::with_capacity(1, OverflowPolicy::Block);
  backend.close();
  assert!(backend.is_closed());
}

#[test]
fn capacity_returns_storage_capacity() {
  let backend = VecDequeBackend::<i32>::with_capacity(10, OverflowPolicy::Block);
  assert_eq!(backend.capacity(), 10);
}

#[test]
fn block_policy_returns_full_error_when_full() {
  let mut backend = VecDequeBackend::with_capacity(1, OverflowPolicy::Block);

  backend.offer(1).unwrap();
  assert!(matches!(backend.offer(2), Err(QueueError::Full(value)) if value == 2));
}

#[test]
fn empty_queue_returns_empty_error() {
  let mut backend = VecDequeBackend::<i32>::with_capacity(5, OverflowPolicy::Block);
  assert!(matches!(backend.poll(), Err(QueueError::Empty)));
}

#[test]
fn grow_policy_doubles_capacity_when_needed() {
  let mut backend = VecDequeBackend::with_capacity(2, OverflowPolicy::Grow);

  backend.offer(1).unwrap();
  backend.offer(2).unwrap();
  let outcome = backend.offer(3).unwrap();
  assert_eq!(outcome, OfferOutcome::GrewTo { capacity: 4 });
  assert_eq!(backend.capacity(), 4);
}

#[test]
fn grow_policy_with_existing_capacity() {
  let mut backend = VecDequeBackend::with_capacity(5, OverflowPolicy::Grow);

  backend.offer(1).unwrap();
  backend.offer(2).unwrap();
  backend.offer(3).unwrap();
  assert_eq!(backend.capacity(), 5);
  assert_eq!(backend.len(), 3);
}

#[test]
fn len_returns_correct_length() {
  let mut backend = VecDequeBackend::with_capacity(5, OverflowPolicy::Block);

  assert_eq!(backend.len(), 0);
  backend.offer(1).unwrap();
  assert_eq!(backend.len(), 1);
  backend.offer(2).unwrap();
  assert_eq!(backend.len(), 2);
  backend.poll().unwrap();
  assert_eq!(backend.len(), 1);
}

#[test]
fn is_empty_when_len_is_zero() {
  let mut backend = VecDequeBackend::with_capacity(5, OverflowPolicy::Block);

  assert_eq!(backend.len(), 0);
  backend.offer(1).unwrap();
  assert_ne!(backend.len(), 0);
  backend.poll().unwrap();
  assert_eq!(backend.len(), 0);
}

#[test]
fn multiple_drop_oldest_operations() {
  let mut backend = VecDequeBackend::with_capacity(2, OverflowPolicy::DropOldest);

  backend.offer(1).unwrap();
  backend.offer(2).unwrap();
  assert_eq!(backend.offer(3).unwrap(), OfferOutcome::DroppedOldest { count: 1 });
  assert_eq!(backend.offer(4).unwrap(), OfferOutcome::DroppedOldest { count: 1 });

  assert_eq!(backend.poll().unwrap(), 3);
  assert_eq!(backend.poll().unwrap(), 4);
}

#[test]
fn multiple_drop_newest_operations() {
  let mut backend = VecDequeBackend::with_capacity(1, OverflowPolicy::DropNewest);

  backend.offer(1).unwrap();
  assert_eq!(backend.offer(2).unwrap(), OfferOutcome::DroppedNewest { count: 1 });
  assert_eq!(backend.offer(3).unwrap(), OfferOutcome::DroppedNewest { count: 1 });

  assert_eq!(backend.poll().unwrap(), 1);
  assert!(matches!(backend.poll(), Err(QueueError::Empty)));
}

#[test]
fn grow_policy_with_large_capacity_increase() {
  let mut backend = VecDequeBackend::<i32>::with_capacity(1, OverflowPolicy::Grow);

  backend.offer(1).unwrap();
  let outcome = backend.offer(2).unwrap();
  assert_eq!(outcome, OfferOutcome::GrewTo { capacity: 2 });

  let outcome = backend.offer(3).unwrap();
  assert_eq!(outcome, OfferOutcome::GrewTo { capacity: 4 });

  let outcome = backend.offer(4).unwrap();
  assert_eq!(outcome, OfferOutcome::Enqueued);

  assert_eq!(backend.capacity(), 4);
  assert_eq!(backend.len(), 4);
}
