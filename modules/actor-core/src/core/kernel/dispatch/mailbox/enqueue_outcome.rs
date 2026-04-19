//! Outcome reported by a successful
//! [`MessageQueue::enqueue`](super::message_queue::MessageQueue::enqueue).
//!
//! Pekko's `BoundedNodeMessageQueue` / `BoundedPriorityMailbox` report the
//! evicted envelope to `deadLetters` when the `DropOldest` strategy kicks
//! in. `EnqueueOutcome` carries that evicted envelope up to the mailbox
//! layer so it can be routed to the dead-letter destination instead of
//! being silently discarded.

use super::envelope::Envelope;

/// Successful outcome of enqueueing into a [`MessageQueue`](super::message_queue::MessageQueue).
///
/// Failed enqueues (closed / full / timeout) are reported via
/// [`SendError`](crate::core::kernel::actor::error::SendError) instead.
#[derive(Debug)]
pub enum EnqueueOutcome {
  /// The envelope was accepted without displacing any existing message.
  Accepted,
  /// The envelope was accepted, but an existing message was evicted to
  /// make room (e.g.
  /// [`MailboxOverflowStrategy::DropOldest`](super::overflow_strategy::MailboxOverflowStrategy)).
  ///
  /// The mailbox layer must forward the carried envelope to the
  /// dead-letter destination.
  Evicted(Envelope),
}
