use core::num::NonZeroUsize;

use super::*;
use crate::core::kernel::{
  actor::{
    error::SendError,
    messaging::AnyMessage,
    props::{MailboxConfigError, MailboxRequirement},
  },
  dispatch::mailbox::{Envelope, MailboxOverflowStrategy, MailboxPolicy, MailboxRegistryError},
};

#[test]
fn register_and_resolve_mailbox() {
  let mut registry = Mailboxes::new();
  registry.ensure_default();
  let config = MailboxConfig::default().with_warn_threshold(None);
  registry.register("custom", config).expect("register mailbox");
  assert!(registry.resolve("custom").is_ok());
}

#[test]
fn register_duplicate_mailbox_fails() {
  let mut registry = Mailboxes::new();
  registry.ensure_default();
  let config = MailboxConfig::default();
  registry.register("dup", config).expect("first register");
  assert!(matches!(registry.register("dup", MailboxConfig::default()), Err(MailboxRegistryError::Duplicate(_))));
}

#[test]
fn ensure_default_mailbox_is_available() {
  let mut registry = Mailboxes::new();
  registry.ensure_default();
  assert!(registry.resolve(DEFAULT_MAILBOX_ID).is_ok());
}

#[test]
fn create_message_queue_uses_registered_mailbox_policy() {
  let mut registry = Mailboxes::new();
  let capacity = NonZeroUsize::new(1).expect("capacity");
  let config = MailboxConfig::new(MailboxPolicy::bounded(capacity, MailboxOverflowStrategy::DropNewest, None));
  registry.register("bounded", config).expect("register mailbox");

  let queue = registry.create_message_queue("bounded").expect("create queue");
  assert!(queue.enqueue(Envelope::new(AnyMessage::new(1_u32))).is_ok());
  let overflow_result = queue.enqueue(Envelope::new(AnyMessage::new(2_u32)));
  let Err(enqueue_error) = overflow_result else {
    panic!("DropNewest overflow must return Err, got {overflow_result:?}");
  };
  assert!(matches!(enqueue_error.error(), SendError::Full(_)));
}

#[test]
fn create_message_queue_rejects_stable_priority_without_generator() {
  let mut registry = Mailboxes::new();
  let config = MailboxConfig::default().with_stable_priority(true);
  registry.register("bad", config).expect("register mailbox");

  let result = registry.create_message_queue("bad");
  assert!(matches!(
    result,
    Err(MailboxRegistryError::InvalidConfig(MailboxConfigError::StablePriorityWithoutGenerator))
  ));
}

#[test]
fn create_message_queue_from_control_aware_requirement() {
  let mut registry = Mailboxes::new();
  let config = MailboxConfig::default().with_requirement(MailboxRequirement::requires_control_aware());
  registry.register("ctrl", config).expect("register mailbox");

  let queue = registry.create_message_queue("ctrl").expect("create queue");
  assert!(queue.enqueue(Envelope::new(AnyMessage::new(42_u32))).is_ok());
  assert!(queue.has_messages());
}

#[test]
fn create_message_queue_rejects_bounded_with_deque() {
  let mut registry = Mailboxes::new();
  let capacity = NonZeroUsize::new(10).expect("capacity");
  let config = MailboxConfig::new(MailboxPolicy::bounded(capacity, MailboxOverflowStrategy::DropNewest, None))
    .with_requirement(MailboxRequirement::requires_deque());
  registry.register("bounded-deque", config).expect("register mailbox");

  let result = registry.create_message_queue("bounded-deque");
  assert!(matches!(result, Err(MailboxRegistryError::InvalidConfig(MailboxConfigError::BoundedWithDeque))));
}
