//! Opt-in deque capability for message queue implementations.

use super::envelope::Envelope;
use crate::actor::error::SendError;

/// Extension trait for message queues that support front-of-queue insertion.
///
/// Queues that implement this trait can prepend envelopes efficiently in O(1)
/// through the deque-only prepend API on [`Mailbox`](super::Mailbox).
pub trait DequeMessageQueue: Send + Sync {
  /// Inserts an envelope at the front of the queue so it is dequeued first.
  ///
  /// # Errors
  ///
  /// Returns [`SendError`] if the envelope cannot be accepted.
  fn enqueue_first(&self, envelope: Envelope) -> Result<(), SendError>;
}
