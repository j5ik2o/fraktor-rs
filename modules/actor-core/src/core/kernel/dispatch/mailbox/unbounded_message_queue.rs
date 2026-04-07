//! Unbounded message queue backed by a growable deque.

#[cfg(test)]
mod tests;

use fraktor_utils_rs::core::collections::queue::QueueError;

use super::{
  QueueStateHandle, envelope::Envelope, mailbox_enqueue_outcome::EnqueueOutcome, message_queue::MessageQueue,
  policy::MailboxPolicy,
};
use crate::core::kernel::actor::error::SendError;

/// Unbounded message queue that grows as needed.
pub struct UnboundedMessageQueue {
  handle: QueueStateHandle<Envelope>,
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
  fn enqueue(&self, envelope: Envelope) -> Result<EnqueueOutcome, SendError> {
    match self.handle.offer(envelope) {
      | Ok(_) => Ok(EnqueueOutcome::Enqueued),
      | Err(error) => Err(super::map_user_envelope_queue_error(error)),
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
