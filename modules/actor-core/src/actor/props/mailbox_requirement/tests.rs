use fraktor_utils_core_rs::core::collections::queue::capabilities::QueueCapabilitySet;

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
