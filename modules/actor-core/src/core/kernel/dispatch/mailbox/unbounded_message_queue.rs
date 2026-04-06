//! Unbounded message queue backed by a growable deque.

#[cfg(test)]
mod tests;

use fraktor_utils_rs::core::collections::queue::QueueError;

use super::{
  QueueStateHandle, mailbox_enqueue_outcome::EnqueueOutcome, message_queue::MessageQueue, policy::MailboxPolicy,
};
use crate::core::kernel::actor::{error::SendError, messaging::AnyMessage};

/// Unbounded message queue that grows as needed.
pub struct UnboundedMessageQueue {
  handle: QueueStateHandle<AnyMessage>,
}

impl UnboundedMessageQueue {
  /// Creates a new unbounded message queue.
  #[must_use]
  pub fn new() -> Self {
    let policy = MailboxPolicy::unbounded(None);
    let handle = QueueStateHandle::new_user(&policy);
    Self { handle }
  }
}

impl Default for UnboundedMessageQueue {
  fn default() -> Self {
    Self::new()
  }
}

impl MessageQueue for UnboundedMessageQueue {
  fn enqueue(&self, message: AnyMessage) -> Result<EnqueueOutcome, SendError> {
    match self.handle.offer(message) {
      | Ok(_) => Ok(EnqueueOutcome::Enqueued),
      | Err(error) => Err(super::map_user_queue_error(error)),
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
