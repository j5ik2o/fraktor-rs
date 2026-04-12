use fraktor_utils_core_rs::core::sync::ArcShared;

use super::*;
use crate::core::kernel::{
  actor::messaging::AnyMessage,
  dispatch::mailbox::{
    Envelope, MessagePriorityGenerator, UnboundedPriorityMessageQueueStateSharedFactory, mailbox_type::MailboxType,
  },
  system::shared_factory::BuiltinSpinSharedFactory,
};

#[test]
fn creates_priority_queue() {
  let pgen: ArcShared<dyn MessagePriorityGenerator> =
    ArcShared::new(|msg: &AnyMessage| -> i32 { msg.payload().downcast_ref::<i32>().copied().unwrap_or(i32::MAX) });
  let state_shared_factory: ArcShared<dyn UnboundedPriorityMessageQueueStateSharedFactory> =
    ArcShared::new(BuiltinSpinSharedFactory::new());
  let factory = UnboundedPriorityMailboxType::new(pgen, state_shared_factory);
  let queue = factory.create();

  queue.enqueue(Envelope::new(AnyMessage::new(30_i32))).expect("enqueue 30");
  queue.enqueue(Envelope::new(AnyMessage::new(10_i32))).expect("enqueue 10");
  queue.enqueue(Envelope::new(AnyMessage::new(20_i32))).expect("enqueue 20");

  let first = queue.dequeue().expect("1番目のデキュー").into_payload();
  assert_eq!(*first.payload().downcast_ref::<i32>().expect("downcast"), 10);
  let second = queue.dequeue().expect("2番目のデキュー").into_payload();
  assert_eq!(*second.payload().downcast_ref::<i32>().expect("downcast"), 20);
  let third = queue.dequeue().expect("3番目のデキュー").into_payload();
  assert_eq!(*third.payload().downcast_ref::<i32>().expect("downcast"), 30);
  assert!(queue.dequeue().is_none());
}
