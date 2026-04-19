//! Unbounded stable-priority message queue backed by a binary heap.
//!
//! Unlike [`super::UnboundedPriorityMessageQueue`], envelopes with equal
//! priority are dequeued in FIFO (insertion) order.

#[cfg(test)]
mod tests;

use alloc::collections::BinaryHeap;

use fraktor_utils_core_rs::core::sync::{ArcShared, DefaultMutex, SharedAccess, SharedLock};

use super::{
  enqueue_outcome::EnqueueOutcome, envelope::Envelope, message_queue::MessageQueue,
  stable_priority_entry::StablePriorityEntry,
};
use crate::core::kernel::{
  actor::error::SendError, dispatch::mailbox::message_priority_generator::MessagePriorityGenerator,
};

/// Initial capacity hint for the backing binary heap.
const DEFAULT_CAPACITY: usize = 11;

/// Internal mutable state guarded by a lock.
struct Inner {
  heap:     BinaryHeap<StablePriorityEntry>,
  sequence: u64,
}

/// Unbounded message queue that dequeues in priority order with stable
/// (FIFO) ordering among envelopes of equal priority.
///
/// Inspired by Pekko's `UnboundedStablePriorityMailbox`.
pub struct UnboundedStablePriorityMessageQueue {
  inner:     SharedLock<Inner>,
  generator: ArcShared<dyn MessagePriorityGenerator>,
}

impl UnboundedStablePriorityMessageQueue {
  /// Creates a new unbounded stable-priority message queue.
  #[must_use]
  pub fn new(generator: ArcShared<dyn MessagePriorityGenerator>) -> Self {
    Self {
      inner: SharedLock::new_with_driver::<DefaultMutex<_>>(Inner {
        heap:     BinaryHeap::with_capacity(DEFAULT_CAPACITY),
        sequence: 0,
      }),
      generator,
    }
  }
}

impl MessageQueue for UnboundedStablePriorityMessageQueue {
  fn enqueue(&self, envelope: Envelope) -> Result<EnqueueOutcome, SendError> {
    let priority = self.generator.priority(envelope.payload());
    self.inner.with_write(|inner| {
      let sequence = inner.sequence;
      inner.sequence += 1;
      inner.heap.push(StablePriorityEntry { priority, sequence, envelope });
    });
    Ok(EnqueueOutcome::Accepted)
  }

  fn dequeue(&self) -> Option<Envelope> {
    self.inner.with_write(|inner| inner.heap.pop().map(|entry| entry.envelope))
  }

  fn number_of_messages(&self) -> usize {
    self.inner.with_read(|inner| inner.heap.len())
  }

  fn clean_up(&self) {
    self.inner.with_write(|inner| inner.heap.clear());
  }
}
