use core::fmt;

/// Error raised when a [`MailboxConfig`](super::MailboxConfig) violates its
/// construction contract.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MailboxConfigError {
  /// `stable_priority` was enabled without attaching a priority generator.
  StablePriorityWithoutGenerator,
  /// Control-aware mailbox requires an unbounded policy (bounded is not supported).
  ControlAwareRequiresUnboundedPolicy,
  /// Priority generator and control-aware are both set, which is not supported.
  PriorityWithControlAware,
  /// Bounded policy with deque requirement is not supported.
  BoundedWithDeque,
}

impl fmt::Display for MailboxConfigError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      | Self::StablePriorityWithoutGenerator => {
        write!(f, "stable_priority requires a priority generator to be attached")
      },
      | Self::ControlAwareRequiresUnboundedPolicy => {
        write!(f, "control-aware mailbox requires an unbounded policy")
      },
      | Self::PriorityWithControlAware => {
        write!(f, "priority generator and control-aware cannot be used together")
      },
      | Self::BoundedWithDeque => {
        write!(f, "bounded policy with deque requirement is not supported")
      },
    }
  }
}
