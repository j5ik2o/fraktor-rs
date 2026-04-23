use core::fmt::{Display, Formatter, Result as FmtResult};

/// Error raised when a [`MailboxConfig`](super::MailboxConfig) violates its
/// construction contract.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MailboxConfigError {
  /// `stable_priority` was enabled without attaching a priority generator.
  StablePriorityWithoutGenerator,
  /// Priority generator and control-aware are both set, which is not supported.
  PriorityWithControlAware,
  /// Priority generator and deque requirement are both set, which is not supported.
  PriorityWithDeque,
  /// Control-aware and deque requirements are both set, which is not supported.
  DequeWithControlAware,
}

impl Display for MailboxConfigError {
  fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
    match self {
      | Self::StablePriorityWithoutGenerator => {
        write!(f, "stable_priority requires a priority generator to be attached")
      },
      | Self::PriorityWithControlAware => {
        write!(f, "priority generator and control-aware cannot be used together")
      },
      | Self::PriorityWithDeque => {
        write!(f, "priority generator and deque requirement cannot be used together")
      },
      | Self::DequeWithControlAware => {
        write!(f, "control-aware and deque requirements cannot be used together")
      },
    }
  }
}
