use core::num::NonZeroUsize;

use crate::mailbox_policy::MailboxPolicy;

/// Mailbox configuration derived from the props builder.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MailboxConfig {
  policy:         MailboxPolicy,
  warn_threshold: Option<NonZeroUsize>,
}

impl MailboxConfig {
  /// Creates a new mailbox configuration.
  #[must_use]
  pub const fn new(policy: MailboxPolicy) -> Self {
    Self { policy, warn_threshold: None }
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

  /// Updates the warning threshold.
  #[must_use]
  pub const fn with_warn_threshold(mut self, threshold: Option<NonZeroUsize>) -> Self {
    self.warn_threshold = threshold;
    self
  }
}

impl Default for MailboxConfig {
  fn default() -> Self {
    MailboxConfig::new(MailboxPolicy::unbounded(None))
  }
}
