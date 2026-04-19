//! Outcome reported by a successful
//! [`MessageQueue::enqueue`](super::message_queue::MessageQueue::enqueue).
//!
//! Pekko's `BoundedNodeMessageQueue` / `BoundedPriorityMailbox` report the
//! evicted / rejected envelope to `deadLetters` from inside the queue and
//! let the caller observe success. `EnqueueOutcome` carries the displaced
//! envelope up to the mailbox layer so it can be routed to the dead-letter
//! destination instead of being silently discarded while preserving Pekko's
//! "enqueue is void" contract: both `DropOldest` (an existing message is
//! evicted to make room) and `DropNewest` (the incoming message is
//! rejected) report success from the caller's perspective — the mailbox
//! layer is the sole dead-letter recorder for overflow.
//!
//! True failures that the caller must observe (closed mailbox, timeout,
//! etc.) are still reported via
//! [`EnqueueError`](super::enqueue_error::EnqueueError).

use super::envelope::Envelope;

/// Successful outcome of enqueueing into a [`MessageQueue`](super::message_queue::MessageQueue).
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
  /// The incoming envelope was rejected because the queue is at capacity
  /// and the policy is
  /// [`MailboxOverflowStrategy::DropNewest`](super::overflow_strategy::MailboxOverflowStrategy).
  ///
  /// The mailbox layer must forward the carried envelope to the
  /// dead-letter destination. The queue state is unchanged.
  Rejected(Envelope),
}
