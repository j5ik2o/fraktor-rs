//! Unbounded deque-based message queue with O(1) front insertion.

#[cfg(test)]
mod tests;

use alloc::collections::VecDeque;

use fraktor_utils_core_rs::core::sync::{DefaultMutex, SharedAccess, SharedLock};

use super::{
  deque_message_queue::DequeMessageQueue, enqueue_error::EnqueueError, enqueue_outcome::EnqueueOutcome,
  envelope::Envelope, message_queue::MessageQueue,
};
use crate::core::kernel::actor::error::SendError;

/// Initial capacity hint for the backing deque.
const DEFAULT_CAPACITY: usize = 16;

/// Unbounded message queue backed by a `VecDeque` that supports O(1) front insertion.
///
/// This queue is selected when [`MailboxRequirement::needs_deque()`] is true,
/// enabling efficient prepend operations for stash-based actors instead of the
/// generic prepend fallback removed from the base [`Mailbox`](super::Mailbox).
pub struct UnboundedDequeMessageQueue {
  inner: SharedLock<VecDeque<Envelope>>,
}

impl UnboundedDequeMessageQueue {
  /// Creates a new unbounded deque message queue.
  #[must_use]
  pub fn new() -> Self {
    Self { inner: SharedLock::new_with_driver::<DefaultMutex<_>>(VecDeque::with_capacity(DEFAULT_CAPACITY)) }
  }
}

impl Default for UnboundedDequeMessageQueue {
  fn default() -> Self {
    Self::new()
  }
}

impl MessageQueue for UnboundedDequeMessageQueue {
  fn enqueue(&self, envelope: Envelope) -> Result<EnqueueOutcome, EnqueueError> {
    self.inner.with_write(|inner| inner.push_back(envelope));
    Ok(EnqueueOutcome::Accepted)
  }

  fn dequeue(&self) -> Option<Envelope> {
    self.inner.with_write(|inner| inner.pop_front())
  }

  fn number_of_messages(&self) -> usize {
    self.inner.with_read(|inner| inner.len())
  }

  fn clean_up(&self) {
    self.inner.with_write(|inner| inner.clear());
  }

  fn as_deque(&self) -> Option<&dyn DequeMessageQueue> {
    Some(self)
  }
}

impl DequeMessageQueue for UnboundedDequeMessageQueue {
  fn enqueue_first(&self, envelope: Envelope) -> Result<(), SendError> {
    self.inner.with_write(|inner| inner.push_front(envelope));
    Ok(())
  }
}
