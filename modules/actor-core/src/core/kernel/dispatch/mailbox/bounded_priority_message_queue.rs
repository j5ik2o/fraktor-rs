//! Bounded priority message queue backed by a binary heap with capacity control.

#[cfg(test)]
mod tests;

use alloc::collections::BinaryHeap;
use core::{cmp::Ordering, num::NonZeroUsize};

use fraktor_utils_core_rs::core::sync::{ArcShared, RuntimeMutex};

use super::{envelope::Envelope, message_queue::MessageQueue, overflow_strategy::MailboxOverflowStrategy};
use crate::core::kernel::{
  actor::error::SendError, dispatch::mailbox::message_priority_generator::MessagePriorityGenerator,
};

/// Internal mutable state guarded by a lock.
struct Inner {
  heap: BinaryHeap<PriorityEntry>,
}

/// Bounded message queue that dequeues envelopes in priority order.
///
/// Inspired by Pekko's `BoundedPriorityMailbox`. A [`MessagePriorityGenerator`]
/// assigns an integer priority to each message; lower values are dequeued first.
/// When the queue reaches capacity, the configured [`MailboxOverflowStrategy`]
/// determines the behaviour.
pub struct BoundedPriorityMessageQueue {
  inner:     RuntimeMutex<Inner>,
  generator: ArcShared<dyn MessagePriorityGenerator>,
  capacity:  usize,
  overflow:  MailboxOverflowStrategy,
}

impl BoundedPriorityMessageQueue {
  /// Creates a new bounded priority message queue.
  #[must_use]
  pub fn new(
    generator: ArcShared<dyn MessagePriorityGenerator>,
    capacity: NonZeroUsize,
    overflow: MailboxOverflowStrategy,
  ) -> Self {
    Self {
      inner: RuntimeMutex::new(Inner { heap: BinaryHeap::with_capacity(capacity.get()) }),
      generator,
      capacity: capacity.get(),
      overflow,
    }
  }
}

impl MessageQueue for BoundedPriorityMessageQueue {
  fn enqueue(&self, envelope: Envelope) -> Result<(), SendError> {
    let priority = self.generator.priority(envelope.payload());
    let mut guard = self.inner.lock();
    let entry = PriorityEntry { priority, envelope };

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

/// Wrapper that orders envelopes by priority for use in [`BinaryHeap`].
///
/// Same ordering semantics as [`super::unbounded_priority_message_queue`]: lower
/// priority values compare as greater so that they are dequeued first from the
/// max-heap.
struct PriorityEntry {
  priority: i32,
  envelope: Envelope,
}

// BinaryHeap での使用を前提としているため、priority のみで比較する。
// envelope の比較は不要（AnyMessage は PartialEq を実装しない）。
impl PartialEq for PriorityEntry {
  fn eq(&self, other: &Self) -> bool {
    self.priority == other.priority
  }
}

impl Eq for PriorityEntry {}

impl PartialOrd for PriorityEntry {
  fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
    Some(self.cmp(other))
  }
}

impl Ord for PriorityEntry {
  fn cmp(&self, other: &Self) -> Ordering {
    // Reverse: lower priority value → greater in heap ordering → dequeued first.
    other.priority.cmp(&self.priority)
  }
}
