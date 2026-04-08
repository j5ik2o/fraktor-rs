//! Unbounded deque-based message queue with O(1) front insertion.

#[cfg(test)]
mod tests;

use alloc::collections::VecDeque;

use fraktor_utils_core_rs::core::sync::RuntimeMutex;

use super::{deque_message_queue::DequeMessageQueue, envelope::Envelope, message_queue::MessageQueue};
use crate::core::kernel::actor::error::SendError;

/// Initial capacity hint for the backing deque.
const DEFAULT_CAPACITY: usize = 16;

/// Unbounded message queue backed by a `VecDeque` that supports O(1) front insertion.
///
/// This queue is selected when [`MailboxRequirement::needs_deque()`] is true,
/// enabling efficient prepend operations for stash-based actors instead of the
/// generic prepend fallback removed from the base [`Mailbox`](super::Mailbox).
pub struct UnboundedDequeMessageQueue {
  inner: RuntimeMutex<VecDeque<Envelope>>,
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
  fn enqueue(&self, envelope: Envelope) -> Result<(), SendError> {
    let mut guard = self.inner.lock();
    guard.push_back(envelope);
    Ok(())
  }

  fn dequeue(&self) -> Option<Envelope> {
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
  fn enqueue_first(&self, envelope: Envelope) -> Result<(), SendError> {
    let mut guard = self.inner.lock();
    guard.push_front(envelope);
    Ok(())
  }
}
