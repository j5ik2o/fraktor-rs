use core::num::NonZeroUsize;

use crate::dispatch::mailbox::{
  bounded_mailbox_type::BoundedMailboxType, mailbox_type::MailboxType, overflow_strategy::MailboxOverflowStrategy,
};

#[test]
fn should_create_bounded_message_queue() {
  let cap = NonZeroUsize::new(5).unwrap();
  let factory = BoundedMailboxType::new(cap, MailboxOverflowStrategy::DropNewest);
  let queue = factory.create();

  assert_eq!(queue.number_of_messages(), 0);
  assert!(!queue.has_messages());
}
