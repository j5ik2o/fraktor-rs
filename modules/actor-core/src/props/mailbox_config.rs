use core::num::NonZeroUsize;

use cellactor_utils_core_rs::collections::queue::capabilities::QueueCapabilityRegistry;

use super::MailboxRequirement;
use crate::mailbox::MailboxPolicy;

#[cfg(test)]
mod tests;

/// Mailbox configuration derived from the props builder.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MailboxConfig {
  policy:         MailboxPolicy,
  warn_threshold: Option<NonZeroUsize>,
  requirement:    MailboxRequirement,
  capabilities:   QueueCapabilityRegistry,
}

impl MailboxConfig {
  /// Creates a new mailbox configuration.
  #[must_use]
  pub const fn new(policy: MailboxPolicy) -> Self {
    Self {
      policy,
      warn_threshold: None,
      requirement: MailboxRequirement::none(),
      capabilities: QueueCapabilityRegistry::with_defaults(),
    }
  }

  /// Returns the mailbox policy.
  #[must_use]
  pub const fn policy(&self) -> MailboxPolicy {
    self.policy
  }

  /// Returns the warning threshold.
  #[must_use]
  pub const fn warn_threshold(&self) -> Option<NonZeroUsize> {
    self.warn_threshold
  }

  /// Returns the mailbox requirement description.
  #[must_use]
  pub const fn requirement(&self) -> MailboxRequirement {
    self.requirement
  }

  /// Returns the configured capability registry.
  #[must_use]
  pub const fn capabilities(&self) -> QueueCapabilityRegistry {
    self.capabilities
  }

  /// Updates the warning threshold.
  #[must_use]
  pub const fn with_warn_threshold(mut self, threshold: Option<NonZeroUsize>) -> Self {
    self.warn_threshold = threshold;
    self
  }

  /// Overrides the mailbox requirement set.
  #[must_use]
  pub const fn with_requirement(mut self, requirement: MailboxRequirement) -> Self {
    self.requirement = requirement;
    self
  }

  /// Overrides the capability registry used to validate requirements.
  #[must_use]
  pub const fn with_capabilities(mut self, registry: QueueCapabilityRegistry) -> Self {
    self.capabilities = registry;
    self
  }
}

impl Default for MailboxConfig {
  fn default() -> Self {
    MailboxConfig::new(MailboxPolicy::unbounded(None))
  }
}
