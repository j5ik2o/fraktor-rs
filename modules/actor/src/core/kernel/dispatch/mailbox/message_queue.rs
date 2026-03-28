//! Abstraction over user message queue implementations.

use super::{deque_message_queue::DequeMessageQueue, mailbox_enqueue_outcome::EnqueueOutcome};
use crate::core::kernel::{error::SendError, messaging::AnyMessage};

/// Pluggable user message queue interface inspired by Pekko's `MessageQueue`.
///
/// Implementations must be thread-safe. The mailbox runtime calls these methods
/// while holding appropriate synchronisation internally.
pub trait MessageQueue: Send + Sync {
  /// Enqueues a user message into the queue.
  ///
  /// # Errors
  ///
  /// Returns [`SendError`] if the message cannot be accepted (full, closed, etc.).
  fn enqueue(&self, message: AnyMessage) -> Result<EnqueueOutcome, SendError>;

  /// Dequeues the next user message, if available.
  fn dequeue(&self) -> Option<AnyMessage>;

  /// Returns the number of messages currently in the queue.
  fn number_of_messages(&self) -> usize;

  /// Returns `true` when at least one message is available.
  fn has_messages(&self) -> bool {
    self.number_of_messages() > 0
  }

  /// Clears remaining messages from the queue during shutdown.
  fn clean_up(&self);

  /// Returns the deque capability when this queue supports front-of-queue insertion.
  ///
  /// The default returns `None`. Override this in deque-capable queue implementations
  /// to enable O(1) prepend in
  /// [`Mailbox::prepend_user_messages`](super::Mailbox::prepend_user_messages).
  fn as_deque(&self) -> Option<&dyn DequeMessageQueue> {
    None
  }
}
