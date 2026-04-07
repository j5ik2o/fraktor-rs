use fraktor_utils_rs::core::sync::ArcShared;

use super::*;
use crate::core::kernel::{
  actor::messaging::AnyMessage,
  dispatch::mailbox::{Envelope, MessagePriorityGenerator, mailbox_type::MailboxType},
};

#[test]
fn creates_stable_priority_queue() {
  let pgen: ArcShared<dyn MessagePriorityGenerator> =
    ArcShared::new(|msg: &AnyMessage| -> i32 { msg.payload().downcast_ref::<i32>().copied().unwrap_or(i32::MAX) });
  let factory = UnboundedStablePriorityMailboxType::new(pgen);
  let queue = factory.create();

  queue.enqueue(Envelope::new(AnyMessage::new(30_i32))).expect("enqueue 30");
  queue.enqueue(Envelope::new(AnyMessage::new(10_i32))).expect("enqueue 10");
  queue.enqueue(Envelope::new(AnyMessage::new(20_i32))).expect("enqueue 20");

  let first = queue.dequeue().expect("dequeue 1st").into_payload();
  assert_eq!(*first.payload().downcast_ref::<i32>().expect("downcast"), 10);
}

#[test]
fn preserves_insertion_order_for_equal_priority() {
  let pgen: ArcShared<dyn MessagePriorityGenerator> = ArcShared::new(|_msg: &AnyMessage| -> i32 { 0 });
  let factory = UnboundedStablePriorityMailboxType::new(pgen);
  let queue = factory.create();

  queue.enqueue(Envelope::new(AnyMessage::new("first"))).expect("enqueue first");
  queue.enqueue(Envelope::new(AnyMessage::new("second"))).expect("enqueue second");
  queue.enqueue(Envelope::new(AnyMessage::new("third"))).expect("enqueue third");

  let first = queue.dequeue().expect("dequeue 1st").into_payload();
  assert_eq!(*first.payload().downcast_ref::<&str>().expect("downcast"), "first");

  let second = queue.dequeue().expect("dequeue 2nd").into_payload();
  assert_eq!(*second.payload().downcast_ref::<&str>().expect("downcast"), "second");

  let third = queue.dequeue().expect("dequeue 3rd").into_payload();
  assert_eq!(*third.payload().downcast_ref::<&str>().expect("downcast"), "third");
}
