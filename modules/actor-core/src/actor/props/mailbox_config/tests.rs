use fraktor_utils_core_rs::core::{
  collections::queue::capabilities::{QueueCapability, QueueCapabilitySet},
  sync::ArcShared,
};

use super::*;
use crate::{
  actor::{messaging::AnyMessage, props::MailboxConfigError},
  dispatch::mailbox::MessagePriorityGenerator,
};

struct ConstPriority;

impl MessagePriorityGenerator for ConstPriority {
  fn priority(&self, _msg: &AnyMessage) -> i32 {
    0
  }
}

#[test]
fn builder_overrides_requirement_and_capabilities() {
  let capability_set = QueueCapabilitySet::defaults().with_deque(false);
  let registry = QueueCapabilityRegistry::new(capability_set);
  let config =
    MailboxConfig::default().with_requirement(MailboxRequirement::requires_deque()).with_capabilities(registry);

  assert!(config.requirement().needs_deque());
  assert!(config.capabilities().ensure(QueueCapability::Deque).is_err());
}

#[test]
fn builder_overrides_warn_threshold_and_factory_metadata() {
  use core::num::NonZeroUsize;

  use crate::dispatch::mailbox::MailboxFactory;

  let threshold = NonZeroUsize::new(7).expect("non-zero threshold");
  let config = MailboxConfig::default().with_warn_threshold(Some(threshold));
  let factory: &dyn MailboxFactory = &config;

  assert_eq!(config.warn_threshold(), Some(threshold));
  assert_eq!(factory.warn_threshold(), Some(threshold));
  assert_eq!(factory.policy(), config.policy());
  assert_eq!(factory.requirement(), config.requirement());
  assert!(factory.capabilities().ensure(QueueCapability::Mpsc).is_ok());
}

#[test]
fn factory_creates_default_queue_and_debug_reports_configuration_shape() {
  use crate::dispatch::mailbox::MailboxFactory;

  let config = MailboxConfig::default();
  let factory: &dyn MailboxFactory = &config;

  let _mailbox_type = factory.mailbox_type();
  let _queue = factory.create_message_queue().expect("default queue");
  let debug = format!("{config:?}");
  assert!(debug.contains("MailboxConfig"));
  assert!(debug.contains("has_priority_generator"));
}

#[test]
fn validate_rejects_stable_priority_without_generator() {
  let config = MailboxConfig::default().with_stable_priority(true);
  assert!(config.stable_priority());
  assert_eq!(config.validate(), Err(MailboxConfigError::StablePriorityWithoutGenerator));
}

#[test]
fn validate_accepts_stable_priority_with_generator() {
  let config =
    MailboxConfig::default().with_priority_generator(ArcShared::new(ConstPriority)).with_stable_priority(true);
  assert!(config.validate().is_ok());
}

#[test]
fn validate_accepts_default_config() {
  let config = MailboxConfig::default();
  assert!(config.validate().is_ok());
}

#[test]
fn validate_accepts_control_aware_with_bounded_policy() {
  use core::num::NonZeroUsize;

  use crate::dispatch::mailbox::{MailboxOverflowStrategy, MailboxPolicy};

  let capacity = NonZeroUsize::new(10).expect("capacity is non-zero");
  let bounded_policy = MailboxPolicy::bounded(capacity, MailboxOverflowStrategy::DropNewest, None);
  let config = MailboxConfig::new(bounded_policy).with_requirement(MailboxRequirement::requires_control_aware());
  assert_eq!(config.validate(), Ok(()));
}

#[test]
fn validate_accepts_control_aware_with_unbounded_policy() {
  let config = MailboxConfig::default().with_requirement(MailboxRequirement::requires_control_aware());
  assert!(config.validate().is_ok());
}

#[test]
fn validate_rejects_priority_with_control_aware() {
  let config = MailboxConfig::default()
    .with_priority_generator(ArcShared::new(ConstPriority))
    .with_requirement(MailboxRequirement::requires_control_aware());
  assert_eq!(config.validate(), Err(MailboxConfigError::PriorityWithControlAware));
}

#[test]
fn validate_rejects_priority_with_deque() {
  let config = MailboxConfig::default()
    .with_priority_generator(ArcShared::new(ConstPriority))
    .with_requirement(MailboxRequirement::requires_deque());
  assert_eq!(config.validate(), Err(MailboxConfigError::PriorityWithDeque));
}

#[test]
fn validate_rejects_deque_with_control_aware() {
  let requirement = MailboxRequirement::requires_deque().with_control_aware();
  let config = MailboxConfig::default().with_requirement(requirement);
  assert_eq!(config.validate(), Err(MailboxConfigError::DequeWithControlAware));
}

#[test]
fn validate_accepts_bounded_with_deque() {
  use core::num::NonZeroUsize;

  use crate::dispatch::mailbox::{MailboxOverflowStrategy, MailboxPolicy};

  let capacity = NonZeroUsize::new(10).expect("capacity is non-zero");
  let bounded_policy = MailboxPolicy::bounded(capacity, MailboxOverflowStrategy::DropNewest, None);
  let config = MailboxConfig::new(bounded_policy).with_requirement(MailboxRequirement::requires_deque());
  assert_eq!(config.validate(), Ok(()));
}
