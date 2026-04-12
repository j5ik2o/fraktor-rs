//! Shared state for bounded priority message queues.

use alloc::collections::BinaryHeap;
use core::{cmp::Ordering, num::NonZeroUsize};

use fraktor_utils_core_rs::core::sync::{SharedAccess, SharedLock};

use super::envelope::Envelope;

/// Mutable state guarded by the bounded-priority mailbox shared wrapper.
pub struct BoundedPriorityMessageQueueState {
  heap: BinaryHeap<BoundedPriorityMessageQueueEntry>,
}

impl BoundedPriorityMessageQueueState {
  /// Creates a state container with the requested initial heap capacity.
  #[must_use]
  pub fn with_capacity(capacity: NonZeroUsize) -> Self {
    Self { heap: BinaryHeap::with_capacity(capacity.get()) }
  }

  #[must_use]
  pub(crate) const fn heap(&self) -> &BinaryHeap<BoundedPriorityMessageQueueEntry> {
    &self.heap
  }

  #[must_use]
  pub(crate) fn heap_mut(&mut self) -> &mut BinaryHeap<BoundedPriorityMessageQueueEntry> {
    &mut self.heap
  }
}

/// Shared wrapper around bounded-priority mailbox state.
pub struct BoundedPriorityMessageQueueStateShared {
  inner: SharedLock<BoundedPriorityMessageQueueState>,
}

impl BoundedPriorityMessageQueueStateShared {
  /// Creates a shared wrapper from an existing shared lock.
  #[must_use]
  pub const fn from_shared(inner: SharedLock<BoundedPriorityMessageQueueState>) -> Self {
    Self { inner }
  }
}

impl Clone for BoundedPriorityMessageQueueStateShared {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}

impl SharedAccess<BoundedPriorityMessageQueueState> for BoundedPriorityMessageQueueStateShared {
  fn with_read<R>(&self, f: impl FnOnce(&BoundedPriorityMessageQueueState) -> R) -> R {
    self.inner.with_read(f)
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut BoundedPriorityMessageQueueState) -> R) -> R {
    self.inner.with_write(f)
  }
}

/// Heap entry for bounded priority message queues.
///
/// Lower numeric priorities are dequeued first, so ordering is reversed for
/// `BinaryHeap`.
pub(crate) struct BoundedPriorityMessageQueueEntry {
  priority: i32,
  envelope: Envelope,
}

impl BoundedPriorityMessageQueueEntry {
  #[must_use]
  pub(crate) const fn new(priority: i32, envelope: Envelope) -> Self {
    Self { priority, envelope }
  }

  #[must_use]
  pub(crate) fn into_envelope(self) -> Envelope {
    self.envelope
  }
}

impl PartialEq for BoundedPriorityMessageQueueEntry {
  fn eq(&self, other: &Self) -> bool {
    self.priority == other.priority
  }
}

impl Eq for BoundedPriorityMessageQueueEntry {}

impl PartialOrd for BoundedPriorityMessageQueueEntry {
  fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
    Some(self.cmp(other))
  }
}

impl Ord for BoundedPriorityMessageQueueEntry {
  fn cmp(&self, other: &Self) -> Ordering {
    other.priority.cmp(&self.priority)
  }
}
