use core::num::NonZeroUsize;

use fraktor_utils_rs::core::sync::ArcShared;

use super::*;
use crate::core::{
  dispatch::mailbox::{MailboxOverflowStrategy, MessagePriorityGenerator, mailbox_type::MailboxType},
  messaging::AnyMessage,
};

#[test]
fn creates_bounded_stable_priority_queue() {
  let pgen: ArcShared<dyn MessagePriorityGenerator> =
    ArcShared::new(|msg: &AnyMessage| -> i32 { msg.payload().downcast_ref::<i32>().copied().unwrap_or(0) });
  let capacity = NonZeroUsize::new(10).unwrap();
  let factory = BoundedStablePriorityMailboxType::new(pgen, capacity, MailboxOverflowStrategy::DropNewest);
  let queue = factory.create();

  queue.enqueue(AnyMessage::new(30_i32)).unwrap();
  queue.enqueue(AnyMessage::new(10_i32)).unwrap();

  let first = queue.dequeue().unwrap();
  assert_eq!(*first.payload().downcast_ref::<i32>().unwrap(), 10);
}

#[test]
fn preserves_insertion_order_for_equal_priority() {
  let pgen: ArcShared<dyn MessagePriorityGenerator> = ArcShared::new(|_msg: &AnyMessage| -> i32 { 0 });
  let capacity = NonZeroUsize::new(10).unwrap();
  let factory = BoundedStablePriorityMailboxType::new(pgen, capacity, MailboxOverflowStrategy::DropNewest);
  let queue = factory.create();

  queue.enqueue(AnyMessage::new("first")).unwrap();
  queue.enqueue(AnyMessage::new("second")).unwrap();

  let first = queue.dequeue().unwrap();
  assert_eq!(*first.payload().downcast_ref::<&str>().unwrap(), "first");

  let second = queue.dequeue().unwrap();
  assert_eq!(*second.payload().downcast_ref::<&str>().unwrap(), "second");
}
