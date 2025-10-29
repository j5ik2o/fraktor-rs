//! Mailbox capacity and overflow policies.

mod overflow;

pub use overflow::OverflowPolicy;

/// Mailbox strategy describing capacity and overflow handling.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MailboxPolicy {
  /// Bounded mailbox with a finite capacity and overflow policy.
  /// Bounded mailbox with explicit capacity and overflow handling.
  Bounded {
    /// Maximum number of user messages stored at once.
    capacity: usize,
    /// Policy applied when the capacity is exceeded.
    overflow: OverflowPolicy,
  },
  /// Unbounded mailbox that grows as needed.
  Unbounded,
}

impl MailboxPolicy {
  /// Creates a bounded mailbox configuration.
  #[must_use]
  pub fn bounded(capacity: usize, overflow: OverflowPolicy) -> Self {
    Self::Bounded { capacity, overflow }
  }

  /// Creates an unbounded mailbox configuration.
  #[must_use]
  pub const fn unbounded() -> Self {
    Self::Unbounded
  }

  /// Returns `true` when the mailbox is bounded.
  #[must_use]
  pub const fn is_bounded(&self) -> bool {
    matches!(self, Self::Bounded { .. })
  }

  /// Returns the configured capacity when bounded.
  #[must_use]
  pub const fn capacity(&self) -> Option<usize> {
    match self {
      | Self::Bounded { capacity, .. } => Some(*capacity),
      | Self::Unbounded => None,
    }
  }

  /// Returns the overflow policy when bounded.
  #[must_use]
  pub const fn overflow(&self) -> Option<OverflowPolicy> {
    match self {
      | Self::Bounded { overflow, .. } => Some(*overflow),
      | Self::Unbounded => None,
    }
  }
}
