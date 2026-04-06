//! Unbounded control-aware message queue with dual-queue prioritisation.

#[cfg(test)]
mod tests;

use alloc::collections::VecDeque;

use fraktor_utils_rs::core::sync::RuntimeMutex;

use super::{mailbox_enqueue_outcome::EnqueueOutcome, message_queue::MessageQueue};
use crate::core::kernel::actor::{error::SendError, messaging::AnyMessage};

/// Initial capacity hint for each backing deque.
const DEFAULT_CAPACITY: usize = 16;

/// Unbounded message queue that prioritises control messages over normal messages.
///
/// Inspired by Pekko's `UnboundedControlAwareMailbox`. Messages created via
/// [`AnyMessage::control`] are routed to a dedicated control queue that is
/// drained before the normal queue during [`dequeue`](MessageQueue::dequeue).
pub struct UnboundedControlAwareMessageQueue {
  inner: RuntimeMutex<Inner>,
}

struct Inner {
  control_queue: VecDeque<AnyMessage>,
  normal_queue:  VecDeque<AnyMessage>,
}

impl UnboundedControlAwareMessageQueue {
  /// Creates a new unbounded control-aware message queue.
  #[must_use]
  pub fn new() -> Self {
    Self {
      inner: RuntimeMutex::new(Inner {
        control_queue: VecDeque::with_capacity(DEFAULT_CAPACITY),
        normal_queue:  VecDeque::with_capacity(DEFAULT_CAPACITY),
      }),
    }
  }
}

impl Default for UnboundedControlAwareMessageQueue {
  fn default() -> Self {
    Self::new()
  }
}

impl MessageQueue for UnboundedControlAwareMessageQueue {
  fn enqueue(&self, message: AnyMessage) -> Result<EnqueueOutcome, SendError> {
    let mut guard = self.inner.lock();
    if message.is_control() {
      guard.control_queue.push_back(message);
    } else {
      guard.normal_queue.push_back(message);
    }
    Ok(EnqueueOutcome::Enqueued)
  }

  fn dequeue(&self) -> Option<AnyMessage> {
    let mut guard = self.inner.lock();
    guard.control_queue.pop_front().or_else(|| guard.normal_queue.pop_front())
  }

  fn number_of_messages(&self) -> usize {
    let guard = self.inner.lock();
    guard.control_queue.len() + guard.normal_queue.len()
  }

  fn clean_up(&self) {
    let mut guard = self.inner.lock();
    guard.control_queue.clear();
    guard.normal_queue.clear();
  }
}
