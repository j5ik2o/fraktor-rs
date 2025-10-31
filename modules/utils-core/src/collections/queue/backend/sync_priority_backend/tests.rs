use super::*;
use crate::collections::{
  PriorityMessage,
  queue::{OfferOutcome, OverflowPolicy, QueueError},
};

#[derive(Clone, Debug, PartialEq, Eq)]
struct TestMessage {
  value:    i32,
  priority: Option<i8>,
}

impl TestMessage {
  const fn new(value: i32, priority: Option<i8>) -> Self {
    Self { value, priority }
  }
}

impl PriorityMessage for TestMessage {
  fn get_priority(&self) -> Option<i8> {
    self.priority
  }
}

#[test]
fn offer_and_poll_returns_priority_order() {
  let mut backend = BinaryHeapPriorityBackend::new_with_capacity(3, OverflowPolicy::DropOldest);

  backend.offer(TestMessage::new(5, Some(1))).unwrap();
  backend.offer(TestMessage::new(2, Some(3))).unwrap();
  backend.offer(TestMessage::new(7, Some(5))).unwrap();

  assert_eq!(backend.len(), 3);
  assert_eq!(backend.poll().unwrap().value, 7);
  assert_eq!(backend.poll().unwrap().value, 2);
  assert_eq!(backend.poll().unwrap().value, 5);
  assert!(matches!(backend.poll(), Err(QueueError::Empty)));
}

#[test]
fn offer_with_none_priority_uses_default() {
  let mut backend = BinaryHeapPriorityBackend::new_with_capacity(3, OverflowPolicy::DropOldest);

  backend.offer(TestMessage::new(10, Some(1))).unwrap();
  backend.offer(TestMessage::new(20, None)).unwrap();
  backend.offer(TestMessage::new(30, Some(6))).unwrap();

  assert_eq!(backend.poll().unwrap().value, 30);
  assert_eq!(backend.poll().unwrap().value, 20);
  assert_eq!(backend.poll().unwrap().value, 10);
}

#[test]
fn drop_newest_discards_incoming_item() {
  let mut backend = BinaryHeapPriorityBackend::new_with_capacity(1, OverflowPolicy::DropNewest);

  backend.offer(TestMessage::new(10, Some(2))).unwrap();
  let outcome = backend.offer(TestMessage::new(5, Some(4))).unwrap();
  assert_eq!(outcome, OfferOutcome::DroppedNewest { count: 1 });
  assert_eq!(backend.poll().unwrap().value, 10);
  assert!(matches!(backend.poll(), Err(QueueError::Empty)));
}

#[test]
fn drop_oldest_replaces_head_element() {
  let mut backend = BinaryHeapPriorityBackend::new_with_capacity(1, OverflowPolicy::DropOldest);

  backend.offer(TestMessage::new(10, Some(1))).unwrap();
  let outcome = backend.offer(TestMessage::new(5, Some(3))).unwrap();
  assert_eq!(outcome, OfferOutcome::DroppedOldest { count: 1 });
  assert_eq!(backend.poll().unwrap().value, 5);
  assert!(matches!(backend.poll(), Err(QueueError::Empty)));
}

#[test]
fn grow_policy_increases_capacity_limit() {
  let mut backend = BinaryHeapPriorityBackend::new_with_capacity(1, OverflowPolicy::Grow);

  backend.offer(TestMessage::new(1, Some(1))).unwrap();
  let outcome = backend.offer(TestMessage::new(2, Some(2))).unwrap();
  assert_eq!(outcome, OfferOutcome::GrewTo { capacity: 2 });
  assert_eq!(backend.capacity(), 2);
}

#[test]
fn closed_backend_behaves_consistently() {
  let mut backend = BinaryHeapPriorityBackend::new_with_capacity(2, OverflowPolicy::Block);

  backend.offer(TestMessage::new(1, Some(1))).unwrap();
  backend.close();
  assert!(matches!(backend.offer(TestMessage::new(3, Some(5))), Err(QueueError::Closed(value)) if value.value == 3));
  assert_eq!(backend.poll().unwrap().value, 1);
  assert!(matches!(backend.poll(), Err(QueueError::Disconnected)));
}

#[test]
fn peek_min_reflects_lowest_priority() {
  let mut backend = BinaryHeapPriorityBackend::new_with_capacity(3, OverflowPolicy::DropOldest);

  backend.offer(TestMessage::new(8, Some(6))).unwrap();
  backend.offer(TestMessage::new(3, Some(1))).unwrap();
  backend.offer(TestMessage::new(5, Some(4))).unwrap();

  assert_eq!(backend.peek_min().map(|msg| msg.value), Some(3));
  assert_eq!(backend.poll().unwrap().value, 8);
  assert_eq!(backend.peek_min().map(|msg| msg.value), Some(3));
  assert_eq!(backend.poll().unwrap().value, 5);
  assert_eq!(backend.peek_min().map(|msg| msg.value), Some(3));
  assert_eq!(backend.poll().unwrap().value, 3);
  assert_eq!(backend.peek_min().map(|msg| msg.value), None);
}
