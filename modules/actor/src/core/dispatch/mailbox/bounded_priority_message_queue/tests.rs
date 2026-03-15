use core::num::NonZeroUsize;

use fraktor_utils_rs::core::sync::ArcShared;

use super::*;
use crate::core::{
  dispatch::mailbox::{MailboxOverflowStrategy, MessagePriorityGenerator, message_queue::MessageQueue},
  messaging::AnyMessage,
};

/// Priority generator that assigns priority based on the i32 payload value.
struct PayloadPriorityGenerator;

impl MessagePriorityGenerator for PayloadPriorityGenerator {
  fn priority(&self, message: &AnyMessage) -> i32 {
    message.payload().downcast_ref::<i32>().copied().unwrap_or(i32::MAX)
  }
}

fn capacity(n: usize) -> NonZeroUsize {
  NonZeroUsize::new(n).unwrap()
}

#[test]
fn dequeues_in_priority_order() {
  let pgen = ArcShared::new(PayloadPriorityGenerator);
  let queue = BoundedPriorityMessageQueue::new(pgen, capacity(10), MailboxOverflowStrategy::DropNewest);

  queue.enqueue(AnyMessage::new(30_i32)).unwrap();
  queue.enqueue(AnyMessage::new(10_i32)).unwrap();
  queue.enqueue(AnyMessage::new(20_i32)).unwrap();

  let first = queue.dequeue().unwrap();
  assert_eq!(*first.payload().downcast_ref::<i32>().unwrap(), 10);

  let second = queue.dequeue().unwrap();
  assert_eq!(*second.payload().downcast_ref::<i32>().unwrap(), 20);

  let third = queue.dequeue().unwrap();
  assert_eq!(*third.payload().downcast_ref::<i32>().unwrap(), 30);
}

#[test]
fn drop_newest_rejects_when_full() {
  let pgen = ArcShared::new(PayloadPriorityGenerator);
  let queue = BoundedPriorityMessageQueue::new(pgen, capacity(2), MailboxOverflowStrategy::DropNewest);

  queue.enqueue(AnyMessage::new(10_i32)).unwrap();
  queue.enqueue(AnyMessage::new(20_i32)).unwrap();
  assert_eq!(queue.number_of_messages(), 2);

  // Third enqueue should fail because capacity is 2.
  let result = queue.enqueue(AnyMessage::new(5_i32));
  assert!(result.is_err());
  assert_eq!(queue.number_of_messages(), 2);
}

#[test]
fn drop_oldest_evicts_earliest_inserted_message() {
  let pgen = ArcShared::new(PayloadPriorityGenerator);
  let queue = BoundedPriorityMessageQueue::new(pgen, capacity(2), MailboxOverflowStrategy::DropOldest);

  // Insert priority 10 first, then priority 30.
  queue.enqueue(AnyMessage::new(10_i32)).unwrap();
  queue.enqueue(AnyMessage::new(30_i32)).unwrap();
  assert_eq!(queue.number_of_messages(), 2);

  // Enqueue priority 20 — should evict priority 10 (oldest inserted).
  queue.enqueue(AnyMessage::new(20_i32)).unwrap();
  assert_eq!(queue.number_of_messages(), 2);

  // Remaining: priority 20 and 30, dequeued in priority order.
  let first = queue.dequeue().unwrap();
  assert_eq!(*first.payload().downcast_ref::<i32>().unwrap(), 20);

  let second = queue.dequeue().unwrap();
  assert_eq!(*second.payload().downcast_ref::<i32>().unwrap(), 30);

  assert!(queue.dequeue().is_none());
}

#[test]
fn grow_ignores_capacity() {
  let pgen = ArcShared::new(PayloadPriorityGenerator);
  let queue = BoundedPriorityMessageQueue::new(pgen, capacity(2), MailboxOverflowStrategy::Grow);

  queue.enqueue(AnyMessage::new(30_i32)).unwrap();
  queue.enqueue(AnyMessage::new(10_i32)).unwrap();
  queue.enqueue(AnyMessage::new(20_i32)).unwrap();
  assert_eq!(queue.number_of_messages(), 3);

  let first = queue.dequeue().unwrap();
  assert_eq!(*first.payload().downcast_ref::<i32>().unwrap(), 10);
}

#[test]
fn block_returns_error() {
  let pgen = ArcShared::new(PayloadPriorityGenerator);
  let queue = BoundedPriorityMessageQueue::new(pgen, capacity(1), MailboxOverflowStrategy::Block);

  queue.enqueue(AnyMessage::new(10_i32)).unwrap();
  let result = queue.enqueue(AnyMessage::new(20_i32));
  assert!(result.is_err());
}

#[test]
fn clean_up_removes_all_messages() {
  let pgen = ArcShared::new(PayloadPriorityGenerator);
  let queue = BoundedPriorityMessageQueue::new(pgen, capacity(10), MailboxOverflowStrategy::DropNewest);

  queue.enqueue(AnyMessage::new(1_i32)).unwrap();
  queue.enqueue(AnyMessage::new(2_i32)).unwrap();
  queue.clean_up();

  assert_eq!(queue.number_of_messages(), 0);
  assert!(queue.dequeue().is_none());
}

#[test]
fn dequeue_empty_returns_none() {
  let pgen = ArcShared::new(PayloadPriorityGenerator);
  let queue = BoundedPriorityMessageQueue::new(pgen, capacity(10), MailboxOverflowStrategy::DropNewest);
  assert!(queue.dequeue().is_none());
}
