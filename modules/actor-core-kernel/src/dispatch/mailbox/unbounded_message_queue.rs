//! Unbounded message queue backed by a mailbox-local lock-free MPSC queue.

#[cfg(test)]
#[path = "unbounded_message_queue_test.rs"]
mod tests;

use super::{
  enqueue_error::EnqueueError, enqueue_outcome::EnqueueOutcome, envelope::Envelope,
  lock_free_mpsc_queue::LockFreeMpscQueue, message_queue::MessageQueue,
};
use crate::actor::error::SendError;

/// Unbounded message queue that grows as needed.
pub struct UnboundedMessageQueue {
  queue: LockFreeMpscQueue<Envelope>,
}

impl UnboundedMessageQueue {
  /// Creates a new unbounded message queue.
  #[must_use]
  #[cfg(not(loom))]
  pub const fn new() -> Self {
    Self { queue: LockFreeMpscQueue::new() }
  }

  /// Creates a new unbounded message queue.
  #[must_use]
  #[cfg(loom)]
  pub fn new() -> Self {
    Self { queue: LockFreeMpscQueue::new() }
  }
}

impl Default for UnboundedMessageQueue {
  fn default() -> Self {
    Self::new()
  }
}

impl MessageQueue for UnboundedMessageQueue {
  fn enqueue(&self, envelope: Envelope) -> Result<EnqueueOutcome, EnqueueError> {
    match self.queue.push(envelope) {
      | Ok(()) => Ok(EnqueueOutcome::Accepted),
      | Err(envelope) => Err(EnqueueError::new(SendError::closed(envelope.into_payload()))),
    }
  }

  fn dequeue(&self) -> Option<Envelope> {
    self.queue.pop()
  }

  fn number_of_messages(&self) -> usize {
    self.queue.len()
  }

  fn clean_up(&self) {
    self.queue.close_and_drain();
  }

  fn close_for_cleanup(&self) {
    self.queue.close();
  }

  fn requires_put_lock_for_enqueue(&self) -> bool {
    false
  }
}
