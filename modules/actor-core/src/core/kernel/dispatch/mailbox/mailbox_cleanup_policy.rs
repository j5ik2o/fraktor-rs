//! Cleanup policy variants for [`Mailbox`](super::Mailbox).

/// Determines how a mailbox treats its user queue when shut down.
///
/// `DrainToDeadLetters` is the default for per-actor mailboxes; the queue is
/// drained and any pending messages are routed to dead letters.
/// `LeaveSharedQueue` is used by `BalancingDispatcher`'s sharing mailboxes:
/// the underlying queue is shared with other team members, so cleanup must
/// not drain it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MailboxCleanupPolicy {
  /// Drain the user queue to dead letters when the mailbox is shut down.
  DrainToDeadLetters,
  /// Leave the shared user queue alone when the mailbox is shut down.
  LeaveSharedQueue,
}
