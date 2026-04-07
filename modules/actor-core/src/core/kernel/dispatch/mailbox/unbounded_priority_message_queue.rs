//! Unbounded priority message queue backed by a binary heap.

#[cfg(test)]
mod tests;

use alloc::collections::BinaryHeap;
use core::cmp::Ordering;

use fraktor_utils_rs::core::sync::{ArcShared, RuntimeMutex};

use super::{envelope::Envelope, mailbox_enqueue_outcome::EnqueueOutcome, message_queue::MessageQueue};
use crate::core::kernel::{
  actor::error::SendError, dispatch::mailbox::message_priority_generator::MessagePriorityGenerator,
};

/// Initial capacity hint for the backing binary heap.
const DEFAULT_CAPACITY: usize = 11;

/// Unbounded message queue that dequeues envelopes in priority order.
///
/// Inspired by Pekko's `UnboundedPriorityMailbox`. A [`MessagePriorityGenerator`]
/// assigns an integer priority to each message; lower values are dequeued first.
pub struct UnboundedPriorityMessageQueue {
  inner:     RuntimeMutex<BinaryHeap<PriorityEntry>>,
  generator: ArcShared<dyn MessagePriorityGenerator>,
}

impl UnboundedPriorityMessageQueue {
  /// Creates a new unbounded priority message queue.
  #[must_use]
  pub fn new(generator: ArcShared<dyn MessagePriorityGenerator>) -> Self {
    Self { inner: RuntimeMutex::new(BinaryHeap::with_capacity(DEFAULT_CAPACITY)), generator }
  }
}

impl MessageQueue for UnboundedPriorityMessageQueue {
  fn enqueue(&self, envelope: Envelope) -> Result<EnqueueOutcome, SendError> {
    let priority = self.generator.priority(envelope.payload());
    let mut guard = self.inner.lock();
    guard.push(PriorityEntry { priority, envelope });
    Ok(EnqueueOutcome::Enqueued)
  }

  fn dequeue(&self) -> Option<Envelope> {
    let mut guard = self.inner.lock();
    guard.pop().map(|entry| entry.envelope)
  }

  fn number_of_messages(&self) -> usize {
    let guard = self.inner.lock();
    guard.len()
  }

  fn clean_up(&self) {
    let mut guard = self.inner.lock();
    guard.clear();
  }
}

/// Wrapper that orders envelopes by priority for use in [`BinaryHeap`].
///
/// `BinaryHeap` is a max-heap, so [`Ord`] is implemented such that entries
/// with *lower* priority values compare as *greater*, ensuring they are
/// dequeued first.
struct PriorityEntry {
  priority: i32,
  envelope: Envelope,
}

// BinaryHeap での使用を前提としているため、priority のみで比較する。
// envelope の比較は不要（AnyMessage は PartialEq を実装しない）。
// 将来この型を公開する場合は比較セマンティクスの再検討が必要。
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
