use fraktor_utils_core_rs::core::collections::queue::capabilities::{QueueCapability, QueueCapabilitySet};

use super::*;

#[test]
fn builder_overrides_requirement_and_capabilities() {
  let capability_set = QueueCapabilitySet::defaults().with_deque(false);
  let registry = QueueCapabilityRegistry::new(capability_set);
  let config =
    MailboxConfig::default().with_requirement(MailboxRequirement::requires_deque()).with_capabilities(registry);

  assert!(config.requirement().needs_deque());
  assert!(config.capabilities().ensure(QueueCapability::Deque).is_err());
}
