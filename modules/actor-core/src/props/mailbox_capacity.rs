//! Capacity strategy describing bounded or unbounded mailboxes.

/// Capacity strategy describing bounded or unbounded mailboxes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MailboxCapacity {
  /// Mailbox stores at most the specified number of messages.
  Bounded(usize),
  /// Mailbox grows without an upper bound.
  Unbounded,
}
