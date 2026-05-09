use fraktor_utils_core_rs::core::sync::ArcShared;

use super::*;
use crate::{
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

/// Priority generator that assigns the same priority to all messages (for insertion-order tests).
struct ConstantPriorityGenerator(i32);

impl MessagePriorityGenerator for ConstantPriorityGenerator {
  fn priority(&self, _message: &AnyMessage) -> i32 {
    self.0
  }
}

#[test]
fn dequeues_in_priority_order() {
  let pgen = ArcShared::new(PayloadPriorityGenerator);
  let queue = UnboundedStablePriorityMessageQueue::new(pgen);

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
fn equal_priority_preserves_insertion_order() {
  let pgen = ArcShared::new(ConstantPriorityGenerator(0));
  let queue = UnboundedStablePriorityMessageQueue::new(pgen);

  queue.enqueue(Envelope::new(AnyMessage::new("first"))).unwrap();
  queue.enqueue(Envelope::new(AnyMessage::new("second"))).unwrap();
  queue.enqueue(Envelope::new(AnyMessage::new("third"))).unwrap();

  let first = queue.dequeue().unwrap().into_payload();
  assert_eq!(*first.payload().downcast_ref::<&str>().unwrap(), "first");

  let second = queue.dequeue().unwrap().into_payload();
  assert_eq!(*second.payload().downcast_ref::<&str>().unwrap(), "second");

  let third = queue.dequeue().unwrap().into_payload();
  assert_eq!(*third.payload().downcast_ref::<&str>().unwrap(), "third");
}

#[test]
fn mixed_priorities_with_stable_ordering() {
  let pgen = ArcShared::new(PayloadPriorityGenerator);
  let queue = UnboundedStablePriorityMessageQueue::new(pgen);

  // 優先度10のメッセージ2つと優先度20のメッセージ2つ
  queue.enqueue(Envelope::new(AnyMessage::new(10_i32))).unwrap();
  queue.enqueue(Envelope::new(AnyMessage::new(20_i32))).unwrap();
  queue.enqueue(Envelope::new(AnyMessage::new(10_i32))).unwrap();
  queue.enqueue(Envelope::new(AnyMessage::new(20_i32))).unwrap();

  // 優先度10が先、挿入順を維持
  let m1 = queue.dequeue().unwrap().into_payload();
  assert_eq!(*m1.payload().downcast_ref::<i32>().unwrap(), 10);

  let m2 = queue.dequeue().unwrap().into_payload();
  assert_eq!(*m2.payload().downcast_ref::<i32>().unwrap(), 10);

  // 次に優先度20、挿入順を維持
  let m3 = queue.dequeue().unwrap().into_payload();
  assert_eq!(*m3.payload().downcast_ref::<i32>().unwrap(), 20);

  let m4 = queue.dequeue().unwrap().into_payload();
  assert_eq!(*m4.payload().downcast_ref::<i32>().unwrap(), 20);
}

#[test]
fn dequeue_empty_returns_none() {
  let pgen = ArcShared::new(PayloadPriorityGenerator);
  let queue = UnboundedStablePriorityMessageQueue::new(pgen);
  assert!(queue.dequeue().is_none());
}

#[test]
fn number_of_messages_tracks_count() {
  let pgen = ArcShared::new(PayloadPriorityGenerator);
  let queue = UnboundedStablePriorityMessageQueue::new(pgen);

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
  let queue = UnboundedStablePriorityMessageQueue::new(pgen);

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
  let queue = UnboundedStablePriorityMessageQueue::new(pgen);

  assert!(!queue.has_messages());
  queue.enqueue(Envelope::new(AnyMessage::new(1_i32))).unwrap();
  assert!(queue.has_messages());
  queue.dequeue();
  assert!(!queue.has_messages());
}
