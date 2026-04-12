use core::num::NonZeroUsize;

use fraktor_utils_core_rs::core::sync::ArcShared;

use super::*;
use crate::core::kernel::{
  actor::messaging::AnyMessage,
  dispatch::mailbox::{
    Envelope, MailboxOverflowStrategy, MessagePriorityGenerator,
    mailbox_type::MailboxType,
  },
};

#[test]
fn creates_bounded_priority_queue() {
  let pgen: ArcShared<dyn MessagePriorityGenerator> =
    ArcShared::new(|msg: &AnyMessage| -> i32 { msg.payload().downcast_ref::<i32>().copied().unwrap_or(i32::MAX) });
  let capacity = NonZeroUsize::new(10).expect("capacity is non-zero");
  let factory =
    BoundedPriorityMailboxType::new(pgen, capacity, MailboxOverflowStrategy::DropNewest);
  let queue = factory.create();

  queue.enqueue(Envelope::new(AnyMessage::new(30_i32))).expect("enqueue 30");
  queue.enqueue(Envelope::new(AnyMessage::new(10_i32))).expect("enqueue 10");

  let first = queue.dequeue().expect("dequeue 1st").into_payload();
  assert_eq!(*first.payload().downcast_ref::<i32>().expect("downcast"), 10);
}
