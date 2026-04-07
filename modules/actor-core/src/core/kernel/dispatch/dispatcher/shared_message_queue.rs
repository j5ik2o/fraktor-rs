//! Shared message queue used by `BalancingDispatcher`.
//!
//! `SharedMessageQueue` is a thread-safe FIFO that multiple actors share to
//! achieve the load-balancing semantics of Pekko's `BalancingDispatcher`. The
//! initial implementation is a plain `RuntimeMutex<VecDeque<Envelope>>`; a
//! lock-free replacement is intentionally out of scope for the
//! dispatcher-pekko-1n-redesign change.

#[cfg(test)]
mod tests;

use alloc::collections::VecDeque;

use fraktor_utils_rs::core::sync::{ArcShared, RuntimeMutex};

use crate::core::kernel::{
  actor::{error::SendError, messaging::AnyMessage},
  dispatch::mailbox::{DequeMessageQueue, EnqueueOutcome, MessageQueue},
};

/// Thread-safe FIFO queue shared by all actors of a `BalancingDispatcher`.
pub struct SharedMessageQueue {
  inner: ArcShared<RuntimeMutex<VecDeque<AnyMessage>>>,
}

impl SharedMessageQueue {
  /// Creates an empty shared message queue.
  #[must_use]
  pub fn new() -> Self {
    Self { inner: ArcShared::new(RuntimeMutex::new(VecDeque::new())) }
  }
}

impl Clone for SharedMessageQueue {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}

impl Default for SharedMessageQueue {
  fn default() -> Self {
    Self::new()
  }
}

impl MessageQueue for SharedMessageQueue {
  fn enqueue(&self, message: AnyMessage) -> Result<EnqueueOutcome, SendError> {
    self.inner.lock().push_back(message);
    Ok(EnqueueOutcome::Enqueued)
  }

  fn dequeue(&self) -> Option<AnyMessage> {
    self.inner.lock().pop_front()
  }

  fn number_of_messages(&self) -> usize {
    self.inner.lock().len()
  }

  fn has_messages(&self) -> bool {
    !self.inner.lock().is_empty()
  }

  fn clean_up(&self) {
    // Sharing semantics: never drain from clean_up. Other team members
    // continue to dequeue from the same queue.
  }

  fn as_deque(&self) -> Option<&dyn DequeMessageQueue> {
    None
  }
}
