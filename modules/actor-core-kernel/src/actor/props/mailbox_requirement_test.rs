use fraktor_utils_core_rs::collections::queue::capabilities::QueueCapabilitySet;

use super::*;

#[test]
fn ensure_supported_detects_missing_capability() {
  let requirement = MailboxRequirement::requires_deque();
  let registry = QueueCapabilityRegistry::new(QueueCapabilitySet::defaults().with_deque(false));
  assert!(matches!(requirement.ensure_supported(&registry), Err(QueueCapabilityError { .. })));
}

#[test]
fn ensure_supported_passes_when_present() {
  let requirement = MailboxRequirement::requires_deque().with_blocking_future();
  let registry = QueueCapabilityRegistry::with_defaults();
  assert!(requirement.ensure_supported(&registry).is_ok());
}

#[test]
fn multiple_consumer_requirement_is_tracked_separately() {
  let requirement = MailboxRequirement::requires_multiple_consumer().with_control_aware();

  assert!(requirement.needs_multiple_consumer());
  assert!(requirement.needs_control_aware());
  assert!(!requirement.needs_deque());
  assert!(!requirement.is_empty());
  assert!(MailboxRequirement::none().is_empty());
}

#[test]
fn with_multiple_consumer_sets_only_multiple_consumer_flag() {
  let requirement = MailboxRequirement::none().with_multiple_consumer();

  assert!(requirement.needs_multiple_consumer());
  assert!(!requirement.needs_deque());
  assert!(!requirement.needs_control_aware());
}
