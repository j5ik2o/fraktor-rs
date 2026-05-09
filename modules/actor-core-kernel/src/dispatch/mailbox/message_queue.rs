//! Abstraction over user message queue implementations.

use super::{
  deque_message_queue::DequeMessageQueue, enqueue_error::EnqueueError, enqueue_outcome::EnqueueOutcome,
  envelope::Envelope,
};

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
  /// Returns one of the success outcomes:
  /// - [`EnqueueOutcome::Accepted`] when the envelope was stored without displacing anything.
  /// - [`EnqueueOutcome::Evicted`] when an existing message was displaced to make room (e.g.
  ///   [`MailboxOverflowStrategy::DropOldest`]).
  /// - [`EnqueueOutcome::Rejected`] when the incoming envelope itself was rejected because the
  ///   queue is at capacity (e.g. [`MailboxOverflowStrategy::DropNewest`]).
  ///
  /// The mailbox layer is responsible for routing both evicted and rejected
  /// envelopes to the dead-letter sink. From the caller's perspective all
  /// three outcomes are "success" (Pekko `BoundedMailbox.enqueue`
  /// void-on-success parity).
  ///
  /// # Errors
  ///
  /// Returns an [`EnqueueError`] only for true enqueue failures that are
  /// **not** overflow: the underlying queue is closed, a non-overflow
  /// rejection is raised (timeout, alloc failure, …). The error may also
  /// carry an evicted envelope surfaced by
  /// [`MailboxOverflowStrategy::DropOldest`] when an eviction happened
  /// before the offer failed; the mailbox layer must still forward such an
  /// evicted envelope to dead letters.
  ///
  /// [`MailboxOverflowStrategy::DropOldest`]: super::overflow_strategy::MailboxOverflowStrategy
  /// [`MailboxOverflowStrategy::DropNewest`]: super::overflow_strategy::MailboxOverflowStrategy
  fn enqueue(&self, envelope: Envelope) -> Result<EnqueueOutcome, EnqueueError>;

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
