use core::num::NonZeroUsize;

use fraktor_utils_core_rs::core::sync::ArcShared;

use super::*;
use crate::core::kernel::{
  actor::{
    error::SendError,
    messaging::AnyMessage,
    props::{MailboxConfigError, MailboxRequirement},
  },
  dispatch::mailbox::{
    BoundedPriorityMessageQueueStateSharedFactory, BoundedStablePriorityMessageQueueStateSharedFactory, Envelope,
    MailboxOverflowStrategy, MailboxPolicy, MailboxRegistryError, UnboundedPriorityMessageQueueStateSharedFactory,
  },
  system::shared_factory::BuiltinSpinSharedFactory,
};

fn bounded_priority_state_shared_factory() -> ArcShared<dyn BoundedPriorityMessageQueueStateSharedFactory> {
  ArcShared::new(BuiltinSpinSharedFactory::new())
}

fn bounded_stable_state_shared_factory() -> ArcShared<dyn BoundedStablePriorityMessageQueueStateSharedFactory> {
  ArcShared::new(BuiltinSpinSharedFactory::new())
}

fn unbounded_priority_state_shared_factory() -> ArcShared<dyn UnboundedPriorityMessageQueueStateSharedFactory> {
  ArcShared::new(BuiltinSpinSharedFactory::new())
}

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

  let queue = registry
    .create_message_queue(
      "bounded",
      &bounded_priority_state_shared_factory(),
      &unbounded_priority_state_shared_factory(),
      &bounded_stable_state_shared_factory(),
    )
    .expect("create queue");
  assert!(queue.enqueue(Envelope::new(AnyMessage::new(1_u32))).is_ok());
  assert!(matches!(queue.enqueue(Envelope::new(AnyMessage::new(2_u32))), Err(SendError::Full(_))));
}

#[test]
fn create_message_queue_rejects_stable_priority_without_generator() {
  let mut registry = Mailboxes::new();
  let config = MailboxConfig::default().with_stable_priority(true);
  registry.register("bad", config).expect("register mailbox");

  let result = registry.create_message_queue(
    "bad",
    &bounded_priority_state_shared_factory(),
    &unbounded_priority_state_shared_factory(),
    &bounded_stable_state_shared_factory(),
  );
  assert!(matches!(
    result,
    Err(MailboxRegistryError::InvalidConfig(MailboxConfigError::StablePriorityWithoutGenerator))
  ));
}

#[test]
fn create_message_queue_from_control_aware_requirement() {
  // control-aware 要件を持つ config から制御認識キューが生成できることを検証
  let mut registry = Mailboxes::new();
  let config = MailboxConfig::default().with_requirement(MailboxRequirement::requires_control_aware());
  registry.register("ctrl", config).expect("register mailbox");

  let queue = registry
    .create_message_queue(
      "ctrl",
      &bounded_priority_state_shared_factory(),
      &unbounded_priority_state_shared_factory(),
      &bounded_stable_state_shared_factory(),
    )
    .expect("create queue");
  // 制御認識キューは通常メッセージも受け入れられる
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

  let result = registry.create_message_queue(
    "bounded-deque",
    &bounded_priority_state_shared_factory(),
    &unbounded_priority_state_shared_factory(),
    &bounded_stable_state_shared_factory(),
  );
  assert!(matches!(result, Err(MailboxRegistryError::InvalidConfig(MailboxConfigError::BoundedWithDeque))));
}
