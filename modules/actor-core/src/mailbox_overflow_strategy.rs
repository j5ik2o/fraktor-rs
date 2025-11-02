//! Overflow strategies for bounded mailboxes.

/// Strategy invoked when a bounded mailbox reaches capacity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MailboxOverflowStrategy {
  /// Drops the newest enqueued message.
  DropNewest,
  /// Drops the oldest message currently stored.
  DropOldest,
  /// Attempts to grow the underlying storage.
  Grow,
  /// Blocks the producer until capacity becomes available.
  Block,
}

#[cfg(test)]
mod tests;
