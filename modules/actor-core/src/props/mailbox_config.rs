//! Mailbox configuration used by Props.

use super::mailbox_capacity::MailboxCapacity;
use crate::mailbox_policy::MailboxPolicy;

/// Mailbox configuration applied when spawning actors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MailboxConfig {
  capacity: MailboxCapacity,
  policy:   MailboxPolicy,
}

impl MailboxConfig {
  /// Creates a bounded mailbox with the provided capacity and policy.
  #[must_use]
  pub const fn bounded(capacity: usize, policy: MailboxPolicy) -> Self {
    Self { capacity: MailboxCapacity::Bounded(capacity), policy }
  }

  /// Creates an unbounded mailbox.
  #[must_use]
  pub const fn unbounded(policy: MailboxPolicy) -> Self {
    Self { capacity: MailboxCapacity::Unbounded, policy }
  }

  /// Returns the configured capacity strategy.
  #[must_use]
  pub const fn capacity(&self) -> MailboxCapacity {
    self.capacity
  }

  /// Returns the overflow policy.
  #[must_use]
  pub const fn policy(&self) -> MailboxPolicy {
    self.policy
  }
}

impl Default for MailboxConfig {
  fn default() -> Self {
    Self::bounded(64, MailboxPolicy::Default)
  }
}
