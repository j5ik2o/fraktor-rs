use crate::dispatch::mailbox::{
  Envelope, mailbox_type::MailboxType, unbounded_control_aware_mailbox_type::UnboundedControlAwareMailboxType,
};

#[test]
fn should_create_working_message_queue() {
  let factory = UnboundedControlAwareMailboxType::new();
  let queue = factory.create();

  assert_eq!(queue.number_of_messages(), 0);
  assert!(!queue.has_messages());
}

#[test]
fn should_create_queue_that_prioritises_control_messages() {
  use crate::actor::messaging::AnyMessage;

  let factory = UnboundedControlAwareMailboxType::new();
  let queue = factory.create();

  queue.enqueue(Envelope::new(AnyMessage::new(1_u32))).unwrap();
  queue.enqueue(Envelope::new(AnyMessage::control(99_u32))).unwrap();

  // Control message should be dequeued first.
  let first = queue.dequeue().unwrap().into_payload();
  assert_eq!(*first.payload().downcast_ref::<u32>().unwrap(), 99);

  let second = queue.dequeue().unwrap().into_payload();
  assert_eq!(*second.payload().downcast_ref::<u32>().unwrap(), 1);
}
