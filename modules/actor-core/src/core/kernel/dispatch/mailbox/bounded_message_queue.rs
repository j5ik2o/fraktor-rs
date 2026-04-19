//! Bounded message queue with configurable overflow strategy.

#[cfg(test)]
mod tests;

use core::num::NonZeroUsize;

use fraktor_utils_core_rs::core::collections::queue::QueueError;

use super::{
  QueueStateHandle, drop_oldest_outcome::DropOldestOutcome, enqueue_outcome::EnqueueOutcome, envelope::Envelope,
  message_queue::MessageQueue, overflow_strategy::MailboxOverflowStrategy, policy::MailboxPolicy,
};
use crate::core::kernel::actor::error::SendError;

/// Bounded message queue with a fixed capacity and configurable overflow behaviour.
pub struct BoundedMessageQueue {
  handle:   QueueStateHandle<Envelope>,
  capacity: usize,
  overflow: MailboxOverflowStrategy,
}

impl BoundedMessageQueue {
  /// Creates a new bounded message queue.
  #[must_use]
  pub fn new(capacity: NonZeroUsize, overflow: MailboxOverflowStrategy) -> Self {
    let policy = MailboxPolicy::bounded(capacity, overflow, None);
    let handle = QueueStateHandle::new_user(&policy);
    Self { handle, capacity: capacity.get(), overflow }
  }
}

impl MessageQueue for BoundedMessageQueue {
  fn enqueue(&self, envelope: Envelope) -> Result<EnqueueOutcome, SendError> {
    match self.overflow {
      | MailboxOverflowStrategy::DropNewest => self.offer_if_room(envelope),
      | MailboxOverflowStrategy::DropOldest => self.offer_after_dropping_oldest(envelope),
      | MailboxOverflowStrategy::Grow => self.offer(envelope),
    }
  }

  fn dequeue(&self) -> Option<Envelope> {
    match self.handle.poll() {
      | Ok(envelope) => Some(envelope),
      | Err(QueueError::Empty | QueueError::Disconnected | QueueError::WouldBlock) => None,
      | Err(_) => None,
    }
  }

  fn number_of_messages(&self) -> usize {
    self.handle.len()
  }

  fn clean_up(&self) {
    while self.dequeue().is_some() {}
  }
}

impl BoundedMessageQueue {
  fn offer(&self, envelope: Envelope) -> Result<EnqueueOutcome, SendError> {
    match self.handle.offer(envelope) {
      | Ok(_) => Ok(EnqueueOutcome::Accepted),
      | Err(error) => Err(super::map_user_envelope_queue_error(error)),
    }
  }

  fn offer_if_room(&self, envelope: Envelope) -> Result<EnqueueOutcome, SendError> {
    match self.handle.offer_if_room(envelope, self.capacity) {
      | Ok(_) => Ok(EnqueueOutcome::Accepted),
      | Err(error) => Err(super::map_user_envelope_queue_error(error)),
    }
  }

  fn offer_after_dropping_oldest(&self, envelope: Envelope) -> Result<EnqueueOutcome, SendError> {
    match self.handle.drop_oldest_and_offer(envelope, self.capacity) {
      | Ok(DropOldestOutcome::Accepted) => Ok(EnqueueOutcome::Accepted),
      | Ok(DropOldestOutcome::Evicted(envelope)) => Ok(EnqueueOutcome::Evicted(envelope)),
      | Err(error) => Err(super::map_user_envelope_queue_error(error)),
    }
  }
}
