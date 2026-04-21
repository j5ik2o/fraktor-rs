//! Event buffer without internal locking.

use alloc::vec::Vec;

use crate::core::kernel::event::stream::{ClassifierKey, EventStreamEvent};

pub(super) const DEFAULT_CAPACITY: usize = 256;

/// Ring buffer-like event storage with trimming to capacity.
pub(crate) struct EventStreamEvents {
  events:   Vec<EventStreamEvent>,
  capacity: usize,
}

impl EventStreamEvents {
  /// Creates an empty buffer with the given capacity.
  #[must_use]
  pub(crate) const fn with_capacity(capacity: usize) -> Self {
    Self { events: Vec::new(), capacity }
  }

  /// Pushes an event and trims the buffer if it exceeds capacity.
  pub(crate) fn push_and_trim(&mut self, event: EventStreamEvent) {
    self.events.push(event);
    if self.events.len() > self.capacity {
      let discard = self.events.len() - self.capacity;
      self.events.drain(0..discard);
    }
  }

  /// Returns a cloned snapshot of buffered events.
  #[must_use]
  pub(crate) fn snapshot(&self) -> Vec<EventStreamEvent> {
    self.events.clone()
  }

  /// Returns buffered events filtered by classifier key.
  #[must_use]
  pub(crate) fn snapshot_for_key(&self, key: ClassifierKey) -> Vec<EventStreamEvent> {
    if key == ClassifierKey::All {
      return self.snapshot();
    }

    self.events.iter().filter(|event| ClassifierKey::for_event(event) == key).cloned().collect()
  }

  /// Capacity accessor.
  #[must_use]
  pub(crate) const fn capacity(&self) -> usize {
    self.capacity
  }
}

impl Default for EventStreamEvents {
  fn default() -> Self {
    Self::with_capacity(DEFAULT_CAPACITY)
  }
}
