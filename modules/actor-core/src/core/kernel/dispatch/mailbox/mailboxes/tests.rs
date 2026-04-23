use core::num::NonZeroUsize;

use super::*;
use crate::core::kernel::{
  actor::{
    messaging::AnyMessage,
    props::{MailboxConfigError, MailboxRequirement},
  },
  dispatch::mailbox::{EnqueueOutcome, Envelope, MailboxOverflowStrategy, MailboxPolicy, MailboxRegistryError},
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
  assert!(
    matches!(overflow_result, Ok(EnqueueOutcome::Rejected(_))),
    "DropNewest overflow must surface Ok(Rejected), got {overflow_result:?}",
  );
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
fn create_message_queue_creates_bounded_deque_for_bounded_plus_deque() {
  let mut registry = Mailboxes::new();
  let capacity = NonZeroUsize::new(2).expect("capacity");
  let config = MailboxConfig::new(MailboxPolicy::bounded(capacity, MailboxOverflowStrategy::DropNewest, None))
    .with_requirement(MailboxRequirement::requires_deque());
  registry.register("bounded-deque", config).expect("register mailbox");

  let queue = registry.create_message_queue("bounded-deque").expect("create queue");
  assert!(queue.as_deque().is_some(), "bounded + deque must expose deque capability");

  // capacity=2 の DropNewest なので 3 件目は Rejected になる。
  queue.enqueue(Envelope::new(AnyMessage::new(1_u32))).expect("enqueue A");
  queue.enqueue(Envelope::new(AnyMessage::new(2_u32))).expect("enqueue B");
  let overflow = queue.enqueue(Envelope::new(AnyMessage::new(3_u32)));
  assert!(
    matches!(overflow, Ok(EnqueueOutcome::Rejected(_))),
    "bounded + deque DropNewest overflow must be Rejected, got {overflow:?}",
  );
}

#[test]
fn create_message_queue_creates_bounded_control_aware_for_bounded_plus_control_aware() {
  let mut registry = Mailboxes::new();
  let capacity = NonZeroUsize::new(2).expect("capacity");
  let config = MailboxConfig::new(MailboxPolicy::bounded(capacity, MailboxOverflowStrategy::DropNewest, None))
    .with_requirement(MailboxRequirement::requires_control_aware());
  registry.register("bounded-control-aware", config).expect("register mailbox");

  let queue = registry.create_message_queue("bounded-control-aware").expect("create queue");

  // control_X と normal_A で capacity=2 を埋めた状態。次の normal は Rejected。
  queue.enqueue(Envelope::new(AnyMessage::control(99_u32))).expect("enqueue control");
  queue.enqueue(Envelope::new(AnyMessage::new(1_u32))).expect("enqueue normal_A");
  let overflow = queue.enqueue(Envelope::new(AnyMessage::new(2_u32)));
  assert!(
    matches!(overflow, Ok(EnqueueOutcome::Rejected(_))),
    "bounded + control_aware DropNewest overflow must be Rejected, got {overflow:?}",
  );

  // control が優先的に dequeue される (control_aware 挙動)。
  let first = queue.dequeue().expect("dequeue 1").into_payload();
  assert!(first.is_control());
  assert_eq!(first.payload().downcast_ref::<u32>().copied(), Some(99_u32));
}

#[test]
fn create_message_queue_honors_custom_mailbox_type_over_policy() {
  use alloc::{boxed::Box, sync::Arc};
  use core::sync::atomic::{AtomicUsize, Ordering};

  use crate::core::kernel::dispatch::mailbox::{MailboxType, MessageQueue, UnboundedMailboxType};

  struct CountingMailboxType {
    create_calls: Arc<AtomicUsize>,
  }
  impl MailboxType for CountingMailboxType {
    fn create(&self) -> Box<dyn MessageQueue> {
      self.create_calls.fetch_add(1, Ordering::SeqCst);
      UnboundedMailboxType::new().create()
    }
  }

  let create_calls = Arc::new(AtomicUsize::new(0));
  let mailbox_type = CountingMailboxType { create_calls: create_calls.clone() };

  // Bounded policy would normally produce a bounded queue. The custom
  // mailbox type must take precedence and be invoked instead.
  let capacity = NonZeroUsize::new(4).expect("capacity");
  let bounded_policy = MailboxPolicy::bounded(capacity, MailboxOverflowStrategy::DropNewest, None);
  let config = MailboxConfig::new(bounded_policy).with_mailbox_type(mailbox_type);

  let mut registry = Mailboxes::new();
  registry.register("custom-type", config).expect("register");
  let _ = registry.create_message_queue("custom-type").expect("create queue via custom type");

  assert_eq!(
    create_calls.load(Ordering::SeqCst),
    1,
    "custom MailboxType::create must be invoked once, regardless of the bounded policy",
  );
}
