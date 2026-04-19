//! Unbounded control-aware message queue with dual-queue prioritisation.

#[cfg(test)]
mod tests;

use alloc::collections::VecDeque;

use fraktor_utils_core_rs::core::sync::{DefaultMutex, SharedAccess, SharedLock};

use super::{enqueue_outcome::EnqueueOutcome, envelope::Envelope, message_queue::MessageQueue};
use crate::core::kernel::actor::error::SendError;

/// Initial capacity hint for each backing deque.
const DEFAULT_CAPACITY: usize = 16;

/// Unbounded message queue that prioritises control messages over normal messages.
///
/// Inspired by Pekko's `UnboundedControlAwareMailbox`. Envelopes whose payload
/// was created via `AnyMessage::control` are routed to a dedicated control
/// queue that is drained before the normal queue during
/// [`dequeue`](MessageQueue::dequeue).
pub struct UnboundedControlAwareMessageQueue {
  inner: SharedLock<Inner>,
}

struct Inner {
  control_queue: VecDeque<Envelope>,
  normal_queue:  VecDeque<Envelope>,
}

impl UnboundedControlAwareMessageQueue {
  /// Creates a new unbounded control-aware message queue.
  #[must_use]
  pub fn new() -> Self {
    Self {
      inner: SharedLock::new_with_driver::<DefaultMutex<_>>(Inner {
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
  fn enqueue(&self, envelope: Envelope) -> Result<EnqueueOutcome, SendError> {
    self.inner.with_write(|inner| {
      if envelope.payload().is_control() {
        inner.control_queue.push_back(envelope);
      } else {
        inner.normal_queue.push_back(envelope);
      }
    });
    Ok(EnqueueOutcome::Accepted)
  }

  fn dequeue(&self) -> Option<Envelope> {
    self.inner.with_write(|inner| inner.control_queue.pop_front().or_else(|| inner.normal_queue.pop_front()))
  }

  fn number_of_messages(&self) -> usize {
    self.inner.with_read(|inner| inner.control_queue.len() + inner.normal_queue.len())
  }

  fn clean_up(&self) {
    self.inner.with_write(|inner| {
      inner.control_queue.clear();
      inner.normal_queue.clear();
    });
  }
}
