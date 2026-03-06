//! Bounded message queue with configurable overflow strategy.

#[cfg(test)]
mod tests;

use core::num::NonZeroUsize;

use fraktor_utils_rs::core::collections::queue::QueueError;

use super::{
  QueueStateHandle, mailbox_enqueue_outcome::EnqueueOutcome, mailbox_offer_future::MailboxOfferFuture,
  message_queue::MessageQueue, overflow_strategy::MailboxOverflowStrategy, policy::MailboxPolicy,
};
use crate::core::{error::SendError, messaging::AnyMessage};

/// Bounded message queue with a fixed capacity and configurable overflow behaviour.
pub struct BoundedMessageQueue {
  handle:   QueueStateHandle<AnyMessage>,
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
  fn enqueue(&self, message: AnyMessage) -> Result<EnqueueOutcome, SendError> {
    match self.overflow {
      | MailboxOverflowStrategy::DropNewest => {
        if self.handle.len() >= self.capacity {
          return Err(SendError::full(message));
        }
        self.offer(message)
      },
      | MailboxOverflowStrategy::DropOldest => {
        if self.handle.len() >= self.capacity {
          let _ = self.handle.poll();
        }
        self.offer(message)
      },
      | MailboxOverflowStrategy::Grow => self.offer(message),
      | MailboxOverflowStrategy::Block => {
        if self.handle.len() >= self.capacity {
          let future = MailboxOfferFuture::new(self.handle.state.clone(), message);
          return Ok(EnqueueOutcome::Pending(future));
        }
        self.offer(message)
      },
    }
  }

  fn dequeue(&self) -> Option<AnyMessage> {
    match self.handle.poll() {
      | Ok(msg) => Some(msg),
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
  fn offer(&self, message: AnyMessage) -> Result<EnqueueOutcome, SendError> {
    match self.handle.offer(message) {
      | Ok(_) => Ok(EnqueueOutcome::Enqueued),
      | Err(error) => Err(super::map_user_queue_error(error)),
    }
  }
}
