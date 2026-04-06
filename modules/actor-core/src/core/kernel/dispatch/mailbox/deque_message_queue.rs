//! Opt-in deque capability for message queue implementations.

use super::mailbox_enqueue_outcome::EnqueueOutcome;
use crate::core::kernel::actor::{error::SendError, messaging::AnyMessage};

/// Extension trait for message queues that support front-of-queue insertion.
///
/// Queues that implement this trait can prepend messages efficiently in O(1)
/// instead of the drain-and-requeue fallback used by the base [`Mailbox`](super::Mailbox).
pub trait DequeMessageQueue: Send + Sync {
  /// Inserts a message at the front of the queue so it is dequeued first.
  ///
  /// # Errors
  ///
  /// Returns [`SendError`] if the message cannot be accepted.
  fn enqueue_first(&self, message: AnyMessage) -> Result<EnqueueOutcome, SendError>;
}
