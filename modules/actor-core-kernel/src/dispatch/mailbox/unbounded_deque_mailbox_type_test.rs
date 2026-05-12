use crate::dispatch::mailbox::{mailbox_type::MailboxType, unbounded_deque_mailbox_type::UnboundedDequeMailboxType};

#[test]
fn should_create_working_message_queue() {
  let factory = UnboundedDequeMailboxType::new();
  let queue = factory.create();

  assert_eq!(queue.number_of_messages(), 0);
  assert!(!queue.has_messages());
}

#[test]
fn should_create_queue_with_deque_capability() {
  let factory = UnboundedDequeMailboxType::new();
  let queue = factory.create();

  assert!(queue.as_deque().is_some());
}
