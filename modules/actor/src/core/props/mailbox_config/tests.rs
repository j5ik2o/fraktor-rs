use fraktor_utils_rs::core::collections::queue::capabilities::{QueueCapability, QueueCapabilitySet};

use super::*;
use crate::core::props::MailboxConfigError;

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
fn validate_rejects_stable_priority_without_generator() {
  let config = MailboxConfig::default().with_stable_priority(true);
  assert_eq!(config.validate(), Err(MailboxConfigError::StablePriorityWithoutGenerator));
}

#[test]
fn validate_accepts_stable_priority_with_generator() {
  use fraktor_utils_rs::core::sync::ArcShared;

  use crate::core::{dispatch::mailbox::MessagePriorityGenerator, messaging::AnyMessage};

  struct ConstPriority;
  impl MessagePriorityGenerator for ConstPriority {
    fn priority(&self, _msg: &AnyMessage) -> i32 {
      0
    }
  }

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
fn validate_rejects_control_aware_with_bounded_policy() {
  use core::num::NonZeroUsize;

  use crate::core::dispatch::mailbox::{MailboxOverflowStrategy, MailboxPolicy};

  let capacity = NonZeroUsize::new(10).unwrap();
  let bounded_policy = MailboxPolicy::bounded(capacity, MailboxOverflowStrategy::DropNewest, None);
  let config = MailboxConfig::new(bounded_policy).with_requirement(MailboxRequirement::requires_control_aware());
  assert_eq!(config.validate(), Err(MailboxConfigError::ControlAwareRequiresUnboundedPolicy));
}

#[test]
fn validate_accepts_control_aware_with_unbounded_policy() {
  let config = MailboxConfig::default().with_requirement(MailboxRequirement::requires_control_aware());
  assert!(config.validate().is_ok());
}

#[test]
fn validate_rejects_priority_with_control_aware() {
  use fraktor_utils_rs::core::sync::ArcShared;

  use crate::core::{dispatch::mailbox::MessagePriorityGenerator, messaging::AnyMessage};

  struct ConstPriority;
  impl MessagePriorityGenerator for ConstPriority {
    fn priority(&self, _msg: &AnyMessage) -> i32 {
      0
    }
  }

  let config = MailboxConfig::default()
    .with_priority_generator(ArcShared::new(ConstPriority))
    .with_requirement(MailboxRequirement::requires_control_aware());
  assert_eq!(config.validate(), Err(MailboxConfigError::PriorityWithControlAware));
}

#[test]
fn validate_rejects_bounded_with_deque() {
  use core::num::NonZeroUsize;

  use crate::core::dispatch::mailbox::{MailboxOverflowStrategy, MailboxPolicy};

  let capacity = NonZeroUsize::new(10).unwrap();
  let bounded_policy = MailboxPolicy::bounded(capacity, MailboxOverflowStrategy::DropNewest, None);
  let config = MailboxConfig::new(bounded_policy).with_requirement(MailboxRequirement::requires_deque());
  assert_eq!(config.validate(), Err(MailboxConfigError::BoundedWithDeque));
}
