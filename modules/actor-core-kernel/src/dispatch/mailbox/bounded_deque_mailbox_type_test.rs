use core::num::NonZeroUsize;

use crate::dispatch::mailbox::{
  bounded_deque_mailbox_type::BoundedDequeMailboxType, mailbox_type::MailboxType,
  overflow_strategy::MailboxOverflowStrategy,
};

#[test]
fn should_create_bounded_deque_message_queue() {
  let cap = NonZeroUsize::new(5).unwrap();
  let factory = BoundedDequeMailboxType::new(cap, MailboxOverflowStrategy::DropNewest);
  let queue = factory.create();

  assert_eq!(queue.number_of_messages(), 0);
  assert!(!queue.has_messages());
}

#[test]
fn should_create_queue_with_deque_capability() {
  let cap = NonZeroUsize::new(5).unwrap();
  let factory = BoundedDequeMailboxType::new(cap, MailboxOverflowStrategy::DropNewest);
  let queue = factory.create();

  assert!(queue.as_deque().is_some());
}
