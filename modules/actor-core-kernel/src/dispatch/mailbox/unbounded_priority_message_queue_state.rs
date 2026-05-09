//! Mutable state for unbounded priority message queues.

use alloc::collections::BinaryHeap;
use core::cmp::Ordering;

use super::envelope::Envelope;

/// Initial capacity hint for the backing binary heap.
pub(crate) const UNBOUNDED_PRIORITY_MESSAGE_QUEUE_DEFAULT_CAPACITY: usize = 11;

/// Mutable state guarded by the unbounded-priority mailbox shared wrapper.
pub struct UnboundedPriorityMessageQueueState {
  heap: BinaryHeap<UnboundedPriorityMessageQueueEntry>,
}

impl UnboundedPriorityMessageQueueState {
  /// Creates a state container with the default initial heap capacity.
  #[must_use]
  pub fn new() -> Self {
    Self { heap: BinaryHeap::with_capacity(UNBOUNDED_PRIORITY_MESSAGE_QUEUE_DEFAULT_CAPACITY) }
  }

  #[must_use]
  pub(crate) const fn heap(&self) -> &BinaryHeap<UnboundedPriorityMessageQueueEntry> {
    &self.heap
  }

  #[must_use]
  pub(crate) const fn heap_mut(&mut self) -> &mut BinaryHeap<UnboundedPriorityMessageQueueEntry> {
    &mut self.heap
  }
}

impl Default for UnboundedPriorityMessageQueueState {
  fn default() -> Self {
    Self::new()
  }
}

/// Heap entry for unbounded priority message queues.
///
/// Lower numeric priorities are dequeued first, so ordering is reversed for
/// `BinaryHeap`.
pub(crate) struct UnboundedPriorityMessageQueueEntry {
  priority: i32,
  envelope: Envelope,
}

impl UnboundedPriorityMessageQueueEntry {
  #[must_use]
  pub(crate) const fn new(priority: i32, envelope: Envelope) -> Self {
    Self { priority, envelope }
  }

  #[must_use]
  pub(crate) fn into_envelope(self) -> Envelope {
    self.envelope
  }
}

impl PartialEq for UnboundedPriorityMessageQueueEntry {
  fn eq(&self, other: &Self) -> bool {
    self.priority == other.priority
  }
}

impl Eq for UnboundedPriorityMessageQueueEntry {}

impl PartialOrd for UnboundedPriorityMessageQueueEntry {
  fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
    Some(self.cmp(other))
  }
}

impl Ord for UnboundedPriorityMessageQueueEntry {
  fn cmp(&self, other: &Self) -> Ordering {
    other.priority.cmp(&self.priority)
  }
}
