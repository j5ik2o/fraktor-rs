//! Heap entry that preserves insertion order among equal-priority messages.

#[cfg(test)]
mod tests;

use core::cmp::Ordering;

use super::envelope::Envelope;

/// Monotonic sequence counter used to break priority ties in FIFO order.
///
/// Each [`super::UnboundedStablePriorityMessageQueue`] and
/// [`super::BoundedStablePriorityMessageQueue`] maintains its own counter,
/// incrementing it on every enqueue.
pub(crate) type SequenceNumber = u64;

/// Heap entry that combines priority, sequence number, and message.
///
/// When used with [`alloc::collections::BinaryHeap`] (a max-heap), the [`Ord`]
/// implementation ensures that:
/// 1. Lower priority values are dequeued first (reversed ordering).
/// 2. Among entries with equal priority, the one with the **smaller** sequence number (inserted
///    earlier) is dequeued first (FIFO / stable ordering).
pub(crate) struct StablePriorityEntry {
  pub(crate) priority: i32,
  pub(crate) sequence: SequenceNumber,
  pub(crate) envelope: Envelope,
}

impl PartialEq for StablePriorityEntry {
  fn eq(&self, other: &Self) -> bool {
    self.priority == other.priority && self.sequence == other.sequence
  }
}

impl Eq for StablePriorityEntry {}

impl PartialOrd for StablePriorityEntry {
  fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
    Some(self.cmp(other))
  }
}

impl Ord for StablePriorityEntry {
  fn cmp(&self, other: &Self) -> Ordering {
    // Primary: lower priority value → greater in heap ordering → dequeued first.
    // Secondary: lower sequence → greater in heap ordering → dequeued first (FIFO).
    other.priority.cmp(&self.priority).then_with(|| other.sequence.cmp(&self.sequence))
  }
}
