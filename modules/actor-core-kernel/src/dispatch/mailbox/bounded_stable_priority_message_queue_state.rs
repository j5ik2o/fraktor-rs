//! Mutable state for bounded stable-priority message queues.

use alloc::collections::BinaryHeap;
use core::num::NonZeroUsize;

use super::stable_priority_entry::StablePriorityEntry;

/// Mutable state guarded by the bounded stable-priority mailbox shared wrapper.
pub struct BoundedStablePriorityMessageQueueState {
  heap:     BinaryHeap<StablePriorityEntry>,
  sequence: u64,
}

impl BoundedStablePriorityMessageQueueState {
  /// Creates a state container with the requested initial heap capacity.
  #[must_use]
  pub fn with_capacity(capacity: NonZeroUsize) -> Self {
    Self { heap: BinaryHeap::with_capacity(capacity.get()), sequence: 0 }
  }

  #[must_use]
  pub(crate) const fn heap(&self) -> &BinaryHeap<StablePriorityEntry> {
    &self.heap
  }

  #[must_use]
  pub(crate) const fn heap_mut(&mut self) -> &mut BinaryHeap<StablePriorityEntry> {
    &mut self.heap
  }

  #[must_use]
  pub(crate) const fn next_sequence(&mut self) -> u64 {
    let sequence = self.sequence;
    self.sequence += 1;
    sequence
  }
}
