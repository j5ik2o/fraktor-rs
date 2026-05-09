use core::num::NonZeroUsize;

use crate::{
  actor::messaging::AnyMessage,
  dispatch::mailbox::{
    Envelope, bounded_control_aware_mailbox_type::BoundedControlAwareMailboxType, mailbox_type::MailboxType,
    overflow_strategy::MailboxOverflowStrategy,
  },
};

#[test]
fn should_create_bounded_control_aware_message_queue() {
  let cap = NonZeroUsize::new(5).unwrap();
  let factory = BoundedControlAwareMailboxType::new(cap, MailboxOverflowStrategy::DropNewest);
  let queue = factory.create();

  assert_eq!(queue.number_of_messages(), 0);
  assert!(!queue.has_messages());
}

#[test]
fn should_create_queue_that_prioritises_control_messages() {
  let cap = NonZeroUsize::new(5).unwrap();
  let factory = BoundedControlAwareMailboxType::new(cap, MailboxOverflowStrategy::DropNewest);
  let queue = factory.create();

  queue.enqueue(Envelope::new(AnyMessage::new(1_u32))).expect("enqueue normal");
  queue.enqueue(Envelope::new(AnyMessage::control(99_u32))).expect("enqueue control");

  let first = queue.dequeue().expect("dequeue 1").into_payload();
  assert_eq!(first.payload().downcast_ref::<u32>().copied(), Some(99_u32));

  let second = queue.dequeue().expect("dequeue 2").into_payload();
  assert_eq!(second.payload().downcast_ref::<u32>().copied(), Some(1_u32));
}
