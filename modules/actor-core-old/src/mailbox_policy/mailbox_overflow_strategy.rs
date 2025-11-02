/// Strategy used when a bounded mailbox reaches capacity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MailboxOverflowStrategy {
  /// Drops the newest message offered to the mailbox.
  DropNewest,
  /// Drops the oldest message stored in the mailbox.
  DropOldest,
  /// Dynamically grows the underlying buffer to accommodate more messages.
  Grow,
  /// Blocks the producer until capacity becomes available.
  Block,
}
