use core::fmt;

/// Error raised when a [`MailboxConfig`](super::MailboxConfig) violates its
/// construction contract.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MailboxConfigError {
  /// `stable_priority` was enabled without attaching a priority generator.
  StablePriorityWithoutGenerator,
}

impl fmt::Display for MailboxConfigError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      | Self::StablePriorityWithoutGenerator => {
        write!(f, "stable_priority requires a priority generator to be attached")
      },
    }
  }
}
