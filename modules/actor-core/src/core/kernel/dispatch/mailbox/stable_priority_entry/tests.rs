use alloc::collections::BinaryHeap;

use crate::core::kernel::{
  actor::messaging::AnyMessage, dispatch::mailbox::stable_priority_entry::StablePriorityEntry,
};

#[test]
fn lower_priority_dequeued_first() {
  let mut heap = BinaryHeap::new();
  heap.push(StablePriorityEntry { priority: 20, sequence: 0, message: AnyMessage::new(20_i32) });
  heap.push(StablePriorityEntry { priority: 10, sequence: 1, message: AnyMessage::new(10_i32) });

  let first = heap.pop().unwrap();
  assert_eq!(first.priority, 10);

  let second = heap.pop().unwrap();
  assert_eq!(second.priority, 20);
}

#[test]
fn equal_priority_preserves_fifo() {
  let mut heap = BinaryHeap::new();
  heap.push(StablePriorityEntry { priority: 5, sequence: 0, message: AnyMessage::new("first") });
  heap.push(StablePriorityEntry { priority: 5, sequence: 1, message: AnyMessage::new("second") });
  heap.push(StablePriorityEntry { priority: 5, sequence: 2, message: AnyMessage::new("third") });

  let first = heap.pop().unwrap();
  assert_eq!(first.sequence, 0);

  let second = heap.pop().unwrap();
  assert_eq!(second.sequence, 1);

  let third = heap.pop().unwrap();
  assert_eq!(third.sequence, 2);
}

#[test]
fn mixed_priorities_with_stable_ordering() {
  let mut heap = BinaryHeap::new();
  heap.push(StablePriorityEntry { priority: 10, sequence: 0, message: AnyMessage::new("a") });
  heap.push(StablePriorityEntry { priority: 5, sequence: 1, message: AnyMessage::new("b") });
  heap.push(StablePriorityEntry { priority: 10, sequence: 2, message: AnyMessage::new("c") });
  heap.push(StablePriorityEntry { priority: 5, sequence: 3, message: AnyMessage::new("d") });

  // Priority 5 first (FIFO within same priority)
  let e1 = heap.pop().unwrap();
  assert_eq!(e1.priority, 5);
  assert_eq!(e1.sequence, 1);

  let e2 = heap.pop().unwrap();
  assert_eq!(e2.priority, 5);
  assert_eq!(e2.sequence, 3);

  // Priority 10 next (FIFO within same priority)
  let e3 = heap.pop().unwrap();
  assert_eq!(e3.priority, 10);
  assert_eq!(e3.sequence, 0);

  let e4 = heap.pop().unwrap();
  assert_eq!(e4.priority, 10);
  assert_eq!(e4.sequence, 2);
}
