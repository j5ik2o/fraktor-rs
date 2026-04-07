//! Bounded stable-priority message queue backed by a binary heap with capacity control.
//!
//! Unlike [`super::BoundedPriorityMessageQueue`], envelopes with equal
//! priority are dequeued in FIFO (insertion) order.

#[cfg(test)]
mod tests;

use alloc::collections::BinaryHeap;
use core::num::NonZeroUsize;

use fraktor_utils_core_rs::core::sync::{ArcShared, RuntimeMutex};

use super::{
  envelope::Envelope, message_queue::MessageQueue, overflow_strategy::MailboxOverflowStrategy,
  stable_priority_entry::StablePriorityEntry,
};
use crate::core::kernel::{
  actor::error::SendError, dispatch::mailbox::message_priority_generator::MessagePriorityGenerator,
};

/// Internal mutable state guarded by a lock.
struct Inner {
  heap:     BinaryHeap<StablePriorityEntry>,
  sequence: u64,
}

/// Bounded message queue that dequeues in priority order with stable
/// (FIFO) ordering among envelopes of equal priority.
///
/// Inspired by Pekko's `BoundedStablePriorityMailbox`. A
/// [`MessagePriorityGenerator`] assigns an integer priority to each message;
/// lower values are dequeued first. When the queue reaches capacity, the
/// configured [`MailboxOverflowStrategy`] determines the behaviour.
pub struct BoundedStablePriorityMessageQueue {
  inner:     RuntimeMutex<Inner>,
  generator: ArcShared<dyn MessagePriorityGenerator>,
  capacity:  usize,
  overflow:  MailboxOverflowStrategy,
}

impl BoundedStablePriorityMessageQueue {
  /// Creates a new bounded stable-priority message queue.
  #[must_use]
  pub fn new(
    generator: ArcShared<dyn MessagePriorityGenerator>,
    capacity: NonZeroUsize,
    overflow: MailboxOverflowStrategy,
  ) -> Self {
    Self {
      inner: RuntimeMutex::new(Inner { heap: BinaryHeap::with_capacity(capacity.get()), sequence: 0 }),
      generator,
      capacity: capacity.get(),
      overflow,
    }
  }
}

impl MessageQueue for BoundedStablePriorityMessageQueue {
  fn enqueue(&self, envelope: Envelope) -> Result<(), SendError> {
    let priority = self.generator.priority(envelope.payload());
    let mut guard = self.inner.lock();
    let sequence = guard.sequence;
    guard.sequence += 1;
    let entry = StablePriorityEntry { priority, sequence, envelope };

    if guard.heap.len() < self.capacity {
      guard.heap.push(entry);
      return Ok(());
    }

    match self.overflow {
      | MailboxOverflowStrategy::DropNewest => {
        // Capacity full — drop the incoming envelope.
        Err(SendError::full(entry.envelope.into_payload()))
      },
      | MailboxOverflowStrategy::DropOldest => {
        // Pekko 互換: キュー先頭（次にデキューされる最高優先度メッセージ）を削除する
        let _ = guard.heap.pop();
        guard.heap.push(entry);
        Ok(())
      },
      | MailboxOverflowStrategy::Grow => {
        // Ignore the bound and grow.
        guard.heap.push(entry);
        Ok(())
      },
    }
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
