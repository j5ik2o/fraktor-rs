//! Actor construction parameters.

use crate::mailbox_policy::MailboxPolicy;

/// Describes actor construction parameters used when creating actor cells.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Props {
  mailbox_policy: MailboxPolicy,
}

impl Props {
  /// Creates props with the provided mailbox policy.
  #[must_use]
  pub const fn new(mailbox_policy: MailboxPolicy) -> Self {
    Self { mailbox_policy }
  }

  /// Returns the configured mailbox policy.
  #[must_use]
  pub const fn mailbox_policy(&self) -> MailboxPolicy {
    self.mailbox_policy
  }
}

impl Default for Props {
  fn default() -> Self {
    Self::new(MailboxPolicy::unbounded(None))
  }
}

#[cfg(test)]
mod tests {
  use crate::mailbox_policy::MailboxPolicy;

  use super::Props;

  #[test]
  fn props_retains_policy() {
    let props = Props::new(MailboxPolicy::unbounded(None));
    assert_eq!(props.mailbox_policy(), MailboxPolicy::unbounded(None));
  }
}
