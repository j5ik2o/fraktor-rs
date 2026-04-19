//! Shared message queue used by `BalancingDispatcher`.
//!
//! `SharedMessageQueue` is a thread-safe FIFO that multiple actors share to
//! achieve the load-balancing semantics of Pekko's `BalancingDispatcher`. The
//! initial implementation is a plain `SharedLock<VecDeque<Envelope>>`; a
//! lock-free replacement is intentionally out of scope for the
//! dispatcher-pekko-1n-redesign change.

#[cfg(test)]
mod tests;

use alloc::collections::VecDeque;

use fraktor_utils_core_rs::core::sync::{DefaultMutex, SharedAccess, SharedLock};

use crate::core::kernel::{
  actor::error::SendError,
  dispatch::mailbox::{DequeMessageQueue, EnqueueOutcome, Envelope, MessageQueue},
};

/// Thread-safe FIFO queue shared by all actors of a `BalancingDispatcher`.
pub struct SharedMessageQueue {
  inner: SharedLock<VecDeque<Envelope>>,
}

impl SharedMessageQueue {
  /// Creates a new empty shared queue using the builtin spin lock backend.
  #[must_use]
  pub fn new() -> Self {
    Self::from_shared_lock(SharedLock::new_with_driver::<DefaultMutex<_>>(VecDeque::new()))
  }

  /// Creates a shared queue from an already materialized shared lock.
  #[must_use]
  pub const fn from_shared_lock(inner: SharedLock<VecDeque<Envelope>>) -> Self {
    Self { inner }
  }
}

impl Default for SharedMessageQueue {
  fn default() -> Self {
    Self::new()
  }
}

impl Clone for SharedMessageQueue {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}

impl MessageQueue for SharedMessageQueue {
  fn enqueue(&self, envelope: Envelope) -> Result<EnqueueOutcome, SendError> {
    self.inner.with_write(|inner| inner.push_back(envelope));
    Ok(EnqueueOutcome::Accepted)
  }

  fn dequeue(&self) -> Option<Envelope> {
    self.inner.with_write(|inner| inner.pop_front())
  }

  fn number_of_messages(&self) -> usize {
    self.inner.with_read(|inner| inner.len())
  }

  fn has_messages(&self) -> bool {
    self.inner.with_read(|inner| !inner.is_empty())
  }

  fn clean_up(&self) {
    // Sharing semantics: never drain from clean_up. Other team members
    // continue to dequeue from the same queue.
  }

  fn as_deque(&self) -> Option<&dyn DequeMessageQueue> {
    None
  }
}
