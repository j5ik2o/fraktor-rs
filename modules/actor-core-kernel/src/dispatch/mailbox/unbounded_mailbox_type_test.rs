use crate::dispatch::mailbox::{mailbox_type::MailboxType, unbounded_mailbox_type::UnboundedMailboxType};

#[test]
fn should_create_working_message_queue() {
  let factory = UnboundedMailboxType::new();
  let queue = factory.create();

  assert_eq!(queue.number_of_messages(), 0);
  assert!(!queue.has_messages());
}
