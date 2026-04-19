//! Abstraction over user message queue implementations.

use super::{deque_message_queue::DequeMessageQueue, enqueue_outcome::EnqueueOutcome, envelope::Envelope};
use crate::core::kernel::actor::error::SendError;

/// Pluggable user message queue interface inspired by Pekko's `MessageQueue`.
///
/// Implementations must be thread-safe. The mailbox runtime calls these methods
/// while holding appropriate synchronisation internally.
///
/// User messages travel through the queue wrapped in an [`Envelope`]; the
/// envelope is the seam where future per-message metadata (sender, priority
/// override, correlation id, …) can be added without changing every queue
/// implementation.
pub trait MessageQueue: Send + Sync {
  /// Enqueues a user envelope into the queue.
  ///
  /// Returns [`EnqueueOutcome::Accepted`] when the envelope was stored
  /// without displacing anything, or [`EnqueueOutcome::Evicted`] when an
  /// existing message was displaced (e.g. `DropOldest`). The mailbox layer
  /// is responsible for routing evicted envelopes to dead letters.
  ///
  /// # Errors
  ///
  /// Returns [`SendError`] if the envelope cannot be accepted (full, closed, etc.).
  fn enqueue(&self, envelope: Envelope) -> Result<EnqueueOutcome, SendError>;

  /// Dequeues the next user envelope, if available.
  fn dequeue(&self) -> Option<Envelope>;

  /// Returns the number of envelopes currently in the queue.
  fn number_of_messages(&self) -> usize;

  /// Returns `true` when at least one envelope is available.
  fn has_messages(&self) -> bool {
    self.number_of_messages() > 0
  }

  /// Clears remaining envelopes from the queue during shutdown.
  fn clean_up(&self);

  /// Returns the deque capability when this queue supports front-of-queue insertion.
  ///
  /// The default returns `None`. Override this in deque-capable queue implementations
  /// to enable O(1) prepend in
  /// [`Mailbox::prepend_user_messages_deque`](super::Mailbox::prepend_user_messages_deque).
  fn as_deque(&self) -> Option<&dyn DequeMessageQueue> {
    None
  }
}
