//! Mailbox policy definitions.

/// Strategies for bounded mailbox overflow handling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MailboxPolicy {
  /// Drop the newest message when the mailbox is full.
  DropNewest,
  /// Drop the oldest message when the mailbox is full.
  DropOldest,
  /// Attempt to grow the underlying storage to accommodate more messages.
  Grow,
  /// Block the producer until capacity becomes available.
  Block,
  /// Default policy used when none is specified.
  Default,
}

impl MailboxPolicy {
  /// Returns `true` if the policy drops the newest message.
  #[must_use]
  pub const fn is_drop_newest(&self) -> bool {
    matches!(self, Self::DropNewest)
  }

  /// Returns `true` if the policy drops the oldest message.
  #[must_use]
  pub const fn is_drop_oldest(&self) -> bool {
    matches!(self, Self::DropOldest)
  }

  /// Returns `true` if the policy grows the mailbox.
  #[must_use]
  pub const fn is_grow(&self) -> bool {
    matches!(self, Self::Grow)
  }

  /// Returns `true` if the policy blocks the producer.
  #[must_use]
  pub const fn is_block(&self) -> bool {
    matches!(self, Self::Block)
  }
}
