//! Unbounded stable-priority message queue backed by a binary heap.
//!
//! Unlike [`super::UnboundedPriorityMessageQueue`], envelopes with equal
//! priority are dequeued in FIFO (insertion) order.

#[cfg(test)]
mod tests;

use alloc::collections::BinaryHeap;

use fraktor_utils_rs::core::sync::{ArcShared, RuntimeMutex};

use super::{
  envelope::Envelope, mailbox_enqueue_outcome::EnqueueOutcome, message_queue::MessageQueue,
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
  inner:     RuntimeMutex<Inner>,
  generator: ArcShared<dyn MessagePriorityGenerator>,
}

impl UnboundedStablePriorityMessageQueue {
  /// Creates a new unbounded stable-priority message queue.
  #[must_use]
  pub fn new(generator: ArcShared<dyn MessagePriorityGenerator>) -> Self {
    Self {
      inner: RuntimeMutex::new(Inner { heap: BinaryHeap::with_capacity(DEFAULT_CAPACITY), sequence: 0 }),
      generator,
    }
  }
}

impl MessageQueue for UnboundedStablePriorityMessageQueue {
  fn enqueue(&self, envelope: Envelope) -> Result<EnqueueOutcome, SendError> {
    let priority = self.generator.priority(envelope.payload());
    let mut guard = self.inner.lock();
    let sequence = guard.sequence;
    guard.sequence += 1;
    guard.heap.push(StablePriorityEntry { priority, sequence, envelope });
    Ok(EnqueueOutcome::Enqueued)
  }

  fn dequeue(&self) -> Option<Envelope> {
    let mut guard = self.inner.lock();
    guard.heap.pop().map(|entry| entry.envelope)
  }

  fn number_of_messages(&self) -> usize {
    let guard = self.inner.lock();
    guard.heap.len()
  }

  fn clean_up(&self) {
    let mut guard = self.inner.lock();
    guard.heap.clear();
  }
}
