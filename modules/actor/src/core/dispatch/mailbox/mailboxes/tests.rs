use core::num::NonZeroUsize;

use super::*;
use crate::core::{
  dispatch::mailbox::{MailboxOverflowStrategy, MailboxPolicy},
  error::SendError,
  messaging::AnyMessage,
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
  assert!(queue.enqueue(AnyMessage::new(1_u32)).is_ok());
  assert!(matches!(queue.enqueue(AnyMessage::new(2_u32)), Err(SendError::Full(_))));
}
