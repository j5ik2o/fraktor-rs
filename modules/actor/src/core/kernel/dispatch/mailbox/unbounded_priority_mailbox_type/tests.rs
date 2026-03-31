use fraktor_utils_rs::core::sync::ArcShared;

use super::*;
use crate::core::kernel::{
  actor::messaging::AnyMessage,
  dispatch::mailbox::{MessagePriorityGenerator, mailbox_type::MailboxType},
};

#[test]
fn creates_priority_queue() {
  let pgen: ArcShared<dyn MessagePriorityGenerator> =
    ArcShared::new(|msg: &AnyMessage| -> i32 { msg.payload().downcast_ref::<i32>().copied().unwrap_or(i32::MAX) });
  let factory = UnboundedPriorityMailboxType::new(pgen);
  let queue = factory.create();

  queue.enqueue(AnyMessage::new(30_i32)).expect("enqueue 30");
  queue.enqueue(AnyMessage::new(10_i32)).expect("enqueue 10");
  queue.enqueue(AnyMessage::new(20_i32)).expect("enqueue 20");

  let first = queue.dequeue().expect("1番目のデキュー");
  assert_eq!(*first.payload().downcast_ref::<i32>().expect("downcast"), 10);
  let second = queue.dequeue().expect("2番目のデキュー");
  assert_eq!(*second.payload().downcast_ref::<i32>().expect("downcast"), 20);
  let third = queue.dequeue().expect("3番目のデキュー");
  assert_eq!(*third.payload().downcast_ref::<i32>().expect("downcast"), 30);
  assert!(queue.dequeue().is_none());
}
