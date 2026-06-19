use alloc::{boxed::Box, sync::Arc};
use core::{
  num::NonZeroUsize,
  sync::atomic::{AtomicUsize, Ordering},
  time::Duration,
};

use fraktor_utils_core_rs::sync::ArcShared;

use crate::{
  actor::{
    messaging::AnyMessage,
    props::{MailboxConfig, MailboxRequirement},
  },
  dispatch::mailbox::{
    MailboxFactory, MailboxOverflowStrategy, MailboxPolicy, MailboxType, Mailboxes, MessagePriorityGenerator,
    MessageQueue, MessageQueueSemantics, ProducesMessageQueue, UnboundedMailboxType,
  },
};

/// Counting MailboxType so tests can observe how many queues a factory
/// produced, and prove that a user-supplied factory actually drives queue
/// construction.
struct CountingMailboxType {
  create_calls: Arc<AtomicUsize>,
}

impl MailboxType for CountingMailboxType {
  fn create(&self) -> Box<dyn MessageQueue> {
    self.create_calls.fetch_add(1, Ordering::SeqCst);
    UnboundedMailboxType::new().create()
  }
}

/// A custom MailboxFactory that wraps a CountingMailboxType and surfaces
/// metadata so the trait default methods can be exercised.
struct CustomMailboxFactory {
  mailbox_type: ArcShared<dyn MailboxType>,
}

struct FactoryPriority;

impl MessagePriorityGenerator for FactoryPriority {
  fn priority(&self, _message: &AnyMessage) -> i32 {
    0
  }
}

impl CustomMailboxFactory {
  fn new(create_calls: Arc<AtomicUsize>) -> Self {
    let mailbox_type: ArcShared<dyn MailboxType> = ArcShared::new(CountingMailboxType { create_calls });
    Self { mailbox_type }
  }
}

impl MailboxFactory for CustomMailboxFactory {
  fn mailbox_type(&self) -> ArcShared<dyn MailboxType> {
    self.mailbox_type.clone()
  }
}

#[test]
fn mailbox_config_bridges_into_mailbox_factory() {
  // `MailboxConfig` implements `MailboxFactory`, so the same registry that
  // stores user-supplied factories also accepts a `MailboxConfig` directly.
  let mut registry = Mailboxes::new();
  registry.register("config-bridge", MailboxConfig::default()).expect("register config as factory");

  let factory = registry.resolve("config-bridge").expect("resolve");
  // Default config uses unbounded policy + no warn threshold.
  assert!(matches!(factory.policy().capacity(), crate::dispatch::mailbox::MailboxCapacity::Unbounded));
  assert!(factory.warn_threshold().is_none());

  // The factory produces a message queue end-to-end through the registry.
  let _queue = registry.create_message_queue("config-bridge").expect("create queue");
}

#[test]
fn custom_mailbox_factory_drives_queue_construction() {
  // Registering a user-defined MailboxFactory causes `create_message_queue`
  // to invoke that factory's MailboxType rather than the built-in
  // policy-based selection.
  let create_calls = Arc::new(AtomicUsize::new(0));
  let factory = CustomMailboxFactory::new(create_calls.clone());

  let mut registry = Mailboxes::new();
  registry.register("custom", factory).expect("register custom factory");

  let _queue = registry.create_message_queue("custom").expect("create queue via custom factory");
  assert_eq!(
    create_calls.load(Ordering::SeqCst),
    1,
    "custom MailboxType::create must be invoked exactly once per queue construction",
  );

  // Re-resolving through the trait-object handle also goes through the
  // same factory instance.
  let _queue2 = registry.create_message_queue("custom").expect("create queue again");
  assert_eq!(create_calls.load(Ordering::SeqCst), 2);
}

#[test]
fn mailbox_factory_default_metadata_is_applied() {
  // A MailboxFactory that only overrides `mailbox_type` inherits unbounded
  // policy, no warn threshold, empty requirement, and a default capability
  // registry from the trait default methods.
  let factory = CustomMailboxFactory::new(Arc::new(AtomicUsize::new(0)));
  assert!(factory.warn_threshold().is_none());
  assert!(!factory.requirement().needs_control_aware());
  assert!(!factory.requirement().needs_deque());
  // `capabilities().with_defaults()` is the declared default.
  let defaults = factory.capabilities();
  let _ = defaults; // smoke test: default constructor must not panic.
  assert_eq!(factory.produced_queue_semantics(), MessageQueueSemantics::unbounded());
}

#[test]
fn mailbox_config_advertises_produced_queue_semantics() {
  let multiple = MailboxConfig::default().with_requirement(MailboxRequirement::requires_multiple_consumer());
  let semantics = multiple.produced_queue_semantics();

  assert!(semantics.is_unbounded());
  assert!(semantics.is_multiple_consumer());
  assert!(semantics.satisfies(MailboxRequirement::requires_multiple_consumer()));
}

#[test]
fn produces_message_queue_trait_delegates_to_mailbox_factory_semantics() {
  let multiple = MailboxConfig::default().with_requirement(MailboxRequirement::requires_multiple_consumer());
  let semantics = ProducesMessageQueue::produced_message_queue(&multiple);

  assert!(semantics.is_unbounded());
  assert!(semantics.is_multiple_consumer());
}

#[test]
fn mailbox_config_advertises_specialized_unbounded_semantics() {
  let priority = MailboxConfig::default().with_priority_generator(ArcShared::new(FactoryPriority));
  let priority_semantics = priority.produced_queue_semantics();
  assert!(priority_semantics.is_unbounded());
  assert!(!priority_semantics.is_multiple_consumer());

  let deque = MailboxConfig::default().with_requirement(MailboxRequirement::requires_deque());
  let deque_semantics = deque.produced_queue_semantics();
  assert!(deque_semantics.is_unbounded());
  assert!(!deque_semantics.is_multiple_consumer());
  assert!(deque_semantics.is_deque_based());

  let control = MailboxConfig::default().with_requirement(MailboxRequirement::requires_control_aware());
  let control_semantics = control.produced_queue_semantics();
  assert!(control_semantics.is_unbounded());
  assert!(!control_semantics.is_multiple_consumer());
  assert!(control_semantics.is_control_aware());
}

#[test]
fn message_queue_semantics_builder_and_default_are_observable() {
  let semantics = MessageQueueSemantics::bounded()
    .with_deque_based(true)
    .with_control_aware(true)
    .with_multiple_consumer(true)
    .with_push_timeout(true);

  assert!(semantics.is_bounded());
  assert!(semantics.is_deque_based());
  assert!(semantics.is_control_aware());
  assert!(semantics.is_multiple_consumer());
  assert!(semantics.has_push_timeout());
  assert_eq!(MessageQueueSemantics::default(), MessageQueueSemantics::unbounded());
}

#[test]
fn mailbox_config_advertises_push_timeout_semantics_for_bounded_policy() {
  let capacity = NonZeroUsize::new(1).expect("capacity");
  let policy =
    MailboxPolicy::bounded(capacity, MailboxOverflowStrategy::DropNewest, None).with_push_timeout(Some(Duration::ZERO));
  let config = MailboxConfig::new(policy);

  let semantics = config.produced_queue_semantics();

  assert!(semantics.has_push_timeout());
  assert!(!semantics.is_multiple_consumer());
  assert!(semantics.satisfies(MailboxRequirement::none().with_blocking_future()));
}
