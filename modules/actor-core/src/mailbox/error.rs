//! Errors that can occur while interacting with the mailbox.

/// Errors that can occur while interacting with the mailbox.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MailboxError {
  /// Mailbox is at capacity and the policy requires blocking.
  WouldBlock,
  /// Mailbox is suspended for user traffic.
  Suspended,
}
