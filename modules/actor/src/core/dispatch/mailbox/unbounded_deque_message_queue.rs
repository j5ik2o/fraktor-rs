//! Unbounded deque-based message queue with O(1) front insertion.

#[cfg(test)]
mod tests;

use alloc::collections::VecDeque;

use fraktor_utils_rs::core::sync::RuntimeMutex;

use super::{
  deque_message_queue::DequeMessageQueue, mailbox_enqueue_outcome::EnqueueOutcome, message_queue::MessageQueue,
};
use crate::core::{error::SendError, messaging::AnyMessage};

/// Initial capacity hint for the backing deque.
const DEFAULT_CAPACITY: usize = 16;

/// Unbounded message queue backed by a `VecDeque` that supports O(1) front insertion.
///
/// This queue is selected when [`MailboxRequirement::needs_deque()`] is true,
/// enabling efficient prepend operations for stash-based actors instead of the
/// drain-and-requeue fallback used by the base [`Mailbox`](super::Mailbox).
pub struct UnboundedDequeMessageQueue {
  inner: RuntimeMutex<VecDeque<AnyMessage>>,
}

impl UnboundedDequeMessageQueue {
  /// Creates a new unbounded deque message queue.
  #[must_use]
  pub fn new() -> Self {
    Self { inner: RuntimeMutex::new(VecDeque::with_capacity(DEFAULT_CAPACITY)) }
  }
}

impl Default for UnboundedDequeMessageQueue {
  fn default() -> Self {
    Self::new()
  }
}

impl MessageQueue for UnboundedDequeMessageQueue {
  fn enqueue(&self, message: AnyMessage) -> Result<EnqueueOutcome, SendError> {
    let mut guard = self.inner.lock();
    guard.push_back(message);
    Ok(EnqueueOutcome::Enqueued)
  }

  fn dequeue(&self) -> Option<AnyMessage> {
    let mut guard = self.inner.lock();
    guard.pop_front()
  }

  fn number_of_messages(&self) -> usize {
    let guard = self.inner.lock();
    guard.len()
  }

  fn clean_up(&self) {
    let mut guard = self.inner.lock();
    guard.clear();
  }

  fn as_deque(&self) -> Option<&dyn DequeMessageQueue> {
    Some(self)
  }
}

impl DequeMessageQueue for UnboundedDequeMessageQueue {
  fn enqueue_first(&self, message: AnyMessage) -> Result<EnqueueOutcome, SendError> {
    let mut guard = self.inner.lock();
    guard.push_front(message);
    Ok(EnqueueOutcome::Enqueued)
  }
}
