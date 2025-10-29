//! Result of enqueuing a message into the mailbox.

/// Result of enqueuing a message into the mailbox.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MailboxEnqueue {
  /// Message stored without side effects.
  Enqueued,
  /// Message stored after dropping the oldest entry.
  DroppedOldest,
  /// Message dropped because it was the newest entry.
  DroppedNewest,
}
