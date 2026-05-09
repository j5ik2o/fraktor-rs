use alloc::{boxed::Box, sync::Arc};
use core::sync::atomic::{AtomicUsize, Ordering};

use fraktor_utils_core_rs::sync::ArcShared;

use super::MailboxFactory;
use crate::{
  actor::props::MailboxConfig,
  dispatch::mailbox::{MailboxType, Mailboxes, MessageQueue, UnboundedMailboxType},
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
}
