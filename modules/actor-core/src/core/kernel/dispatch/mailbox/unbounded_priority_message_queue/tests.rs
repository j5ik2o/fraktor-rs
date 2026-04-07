use fraktor_utils_rs::core::sync::ArcShared;

use super::*;
use crate::core::kernel::{
  actor::messaging::AnyMessage,
  dispatch::mailbox::{MessagePriorityGenerator, envelope::Envelope, message_queue::MessageQueue},
};

/// Priority generator that assigns priority based on the i32 payload value.
struct PayloadPriorityGenerator;

impl MessagePriorityGenerator for PayloadPriorityGenerator {
  fn priority(&self, message: &AnyMessage) -> i32 {
    message.payload().downcast_ref::<i32>().copied().unwrap_or(i32::MAX)
  }
}

#[test]
fn dequeues_in_priority_order() {
  let pgen = ArcShared::new(PayloadPriorityGenerator);
  let queue = UnboundedPriorityMessageQueue::new(pgen);

  queue.enqueue(Envelope::new(AnyMessage::new(30_i32))).unwrap();
  queue.enqueue(Envelope::new(AnyMessage::new(10_i32))).unwrap();
  queue.enqueue(Envelope::new(AnyMessage::new(20_i32))).unwrap();

  let first = queue.dequeue().unwrap().into_payload();
  assert_eq!(*first.payload().downcast_ref::<i32>().unwrap(), 10);

  let second = queue.dequeue().unwrap().into_payload();
  assert_eq!(*second.payload().downcast_ref::<i32>().unwrap(), 20);

  let third = queue.dequeue().unwrap().into_payload();
  assert_eq!(*third.payload().downcast_ref::<i32>().unwrap(), 30);
}

#[test]
fn dequeue_empty_returns_none() {
  let pgen = ArcShared::new(PayloadPriorityGenerator);
  let queue = UnboundedPriorityMessageQueue::new(pgen);
  assert!(queue.dequeue().is_none());
}

#[test]
fn number_of_messages_tracks_count() {
  let pgen = ArcShared::new(PayloadPriorityGenerator);
  let queue = UnboundedPriorityMessageQueue::new(pgen);

  assert_eq!(queue.number_of_messages(), 0);
  queue.enqueue(Envelope::new(AnyMessage::new(1_i32))).unwrap();
  assert_eq!(queue.number_of_messages(), 1);
  queue.enqueue(Envelope::new(AnyMessage::new(2_i32))).unwrap();
  assert_eq!(queue.number_of_messages(), 2);

  queue.dequeue();
  assert_eq!(queue.number_of_messages(), 1);
}

#[test]
fn clean_up_removes_all_messages() {
  let pgen = ArcShared::new(PayloadPriorityGenerator);
  let queue = UnboundedPriorityMessageQueue::new(pgen);

  queue.enqueue(Envelope::new(AnyMessage::new(1_i32))).unwrap();
  queue.enqueue(Envelope::new(AnyMessage::new(2_i32))).unwrap();
  queue.enqueue(Envelope::new(AnyMessage::new(3_i32))).unwrap();
  assert_eq!(queue.number_of_messages(), 3);

  queue.clean_up();
  assert_eq!(queue.number_of_messages(), 0);
  assert!(queue.dequeue().is_none());
}

#[test]
fn has_messages_reflects_queue_state() {
  let pgen = ArcShared::new(PayloadPriorityGenerator);
  let queue = UnboundedPriorityMessageQueue::new(pgen);

  assert!(!queue.has_messages());
  queue.enqueue(Envelope::new(AnyMessage::new(1_i32))).unwrap();
  assert!(queue.has_messages());
  queue.dequeue();
  assert!(!queue.has_messages());
}

#[test]
fn struct_based_priority_generator_dequeues_in_order() {
  let pgen = ArcShared::new(PayloadPriorityGenerator);
  let queue = UnboundedPriorityMessageQueue::new(pgen);

  queue.enqueue(Envelope::new(AnyMessage::new(50_i32))).expect("enqueue 50");
  queue.enqueue(Envelope::new(AnyMessage::new(5_i32))).expect("enqueue 5");

  let first = queue.dequeue().expect("dequeue 1st").into_payload();
  assert_eq!(*first.payload().downcast_ref::<i32>().expect("downcast"), 5);
}
