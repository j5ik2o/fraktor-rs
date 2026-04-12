use core::num::NonZeroUsize;

use fraktor_utils_core_rs::core::sync::ArcShared;

use super::*;
use crate::core::kernel::{
  actor::messaging::AnyMessage,
  dispatch::mailbox::{
    BoundedPriorityMessageQueueState, BoundedPriorityMessageQueueStateShared, MailboxOverflowStrategy,
    MessagePriorityGenerator, envelope::Envelope, message_queue::MessageQueue,
  },
};

/// Priority generator that assigns priority based on the i32 payload value.
struct PayloadPriorityGenerator;

impl MessagePriorityGenerator for PayloadPriorityGenerator {
  fn priority(&self, message: &AnyMessage) -> i32 {
    message.payload().downcast_ref::<i32>().copied().unwrap_or(i32::MAX)
  }
}

fn capacity(n: usize) -> NonZeroUsize {
  NonZeroUsize::new(n).expect("capacity must be greater than 0")
}

fn queue(
  generator: ArcShared<dyn MessagePriorityGenerator>,
  capacity: NonZeroUsize,
  overflow: MailboxOverflowStrategy,
) -> BoundedPriorityMessageQueue {
  let state_shared =
    BoundedPriorityMessageQueueStateShared::new(BoundedPriorityMessageQueueState::with_capacity(capacity));
  BoundedPriorityMessageQueue::new(generator, state_shared, capacity, overflow)
}

#[test]
fn dequeues_in_priority_order() {
  let pgen = ArcShared::new(PayloadPriorityGenerator);
  let queue = queue(pgen, capacity(10), MailboxOverflowStrategy::DropNewest);

  queue.enqueue(Envelope::new(AnyMessage::new(30_i32))).expect("enqueue 30");
  queue.enqueue(Envelope::new(AnyMessage::new(10_i32))).expect("enqueue 10");
  queue.enqueue(Envelope::new(AnyMessage::new(20_i32))).expect("enqueue 20");

  let first = queue.dequeue().expect("dequeue 1st").into_payload();
  assert_eq!(*first.payload().downcast_ref::<i32>().expect("downcast"), 10);

  let second = queue.dequeue().expect("dequeue 2nd").into_payload();
  assert_eq!(*second.payload().downcast_ref::<i32>().expect("downcast"), 20);

  let third = queue.dequeue().expect("dequeue 3rd").into_payload();
  assert_eq!(*third.payload().downcast_ref::<i32>().expect("downcast"), 30);
}

#[test]
fn drop_newest_rejects_when_full() {
  let pgen = ArcShared::new(PayloadPriorityGenerator);
  let queue = queue(pgen, capacity(2), MailboxOverflowStrategy::DropNewest);

  queue.enqueue(Envelope::new(AnyMessage::new(10_i32))).expect("enqueue 10");
  queue.enqueue(Envelope::new(AnyMessage::new(20_i32))).expect("enqueue 20");
  assert_eq!(queue.number_of_messages(), 2);

  let result = queue.enqueue(Envelope::new(AnyMessage::new(5_i32)));
  assert!(result.is_err());
  assert_eq!(queue.number_of_messages(), 2);
}

#[test]
fn drop_oldest_evicts_earliest_inserted_message() {
  let pgen = ArcShared::new(PayloadPriorityGenerator);
  let queue = queue(pgen, capacity(2), MailboxOverflowStrategy::DropOldest);

  queue.enqueue(Envelope::new(AnyMessage::new(10_i32))).expect("enqueue 10");
  queue.enqueue(Envelope::new(AnyMessage::new(30_i32))).expect("enqueue 30");
  assert_eq!(queue.number_of_messages(), 2);

  queue.enqueue(Envelope::new(AnyMessage::new(20_i32))).expect("enqueue 20");
  assert_eq!(queue.number_of_messages(), 2);

  let first = queue.dequeue().expect("dequeue 1st").into_payload();
  assert_eq!(*first.payload().downcast_ref::<i32>().expect("downcast"), 20);

  let second = queue.dequeue().expect("dequeue 2nd").into_payload();
  assert_eq!(*second.payload().downcast_ref::<i32>().expect("downcast"), 30);

  assert!(queue.dequeue().is_none());
}

#[test]
fn grow_ignores_capacity() {
  let pgen = ArcShared::new(PayloadPriorityGenerator);
  let queue = queue(pgen, capacity(2), MailboxOverflowStrategy::Grow);

  queue.enqueue(Envelope::new(AnyMessage::new(30_i32))).expect("enqueue 30");
  queue.enqueue(Envelope::new(AnyMessage::new(10_i32))).expect("enqueue 10");
  queue.enqueue(Envelope::new(AnyMessage::new(20_i32))).expect("enqueue 20");
  assert_eq!(queue.number_of_messages(), 3);

  let first = queue.dequeue().expect("dequeue 1st").into_payload();
  assert_eq!(*first.payload().downcast_ref::<i32>().expect("downcast"), 10);
}

#[test]
fn clean_up_removes_all_messages() {
  let pgen = ArcShared::new(PayloadPriorityGenerator);
  let queue = queue(pgen, capacity(10), MailboxOverflowStrategy::DropNewest);

  queue.enqueue(Envelope::new(AnyMessage::new(1_i32))).expect("enqueue 1");
  queue.enqueue(Envelope::new(AnyMessage::new(2_i32))).expect("enqueue 2");
  queue.clean_up();

  assert_eq!(queue.number_of_messages(), 0);
  assert!(queue.dequeue().is_none());
}

#[test]
fn dequeue_empty_returns_none() {
  let pgen = ArcShared::new(PayloadPriorityGenerator);
  let queue = queue(pgen, capacity(10), MailboxOverflowStrategy::DropNewest);
  assert!(queue.dequeue().is_none());
}
