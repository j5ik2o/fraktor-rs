//! Event buffer without internal locking.

use alloc::vec::Vec;

use fraktor_utils_rs::core::runtime_toolbox::RuntimeToolbox;

use crate::core::event_stream::EventStreamEvent;

pub(super) const DEFAULT_CAPACITY: usize = 256;

/// Ring buffer-like event storage with trimming to capacity.
pub struct EventStreamEventsGeneric<TB: RuntimeToolbox + 'static> {
  events:   Vec<EventStreamEvent<TB>>,
  capacity: usize,
}

impl<TB: RuntimeToolbox + 'static> EventStreamEventsGeneric<TB> {
  /// Creates an empty buffer with the given capacity.
  #[must_use]
  pub const fn with_capacity(capacity: usize) -> Self {
    Self { events: Vec::new(), capacity }
  }

  /// Pushes an event and trims the buffer if it exceeds capacity.
  pub fn push_and_trim(&mut self, event: EventStreamEvent<TB>) {
    self.events.push(event);
    if self.events.len() > self.capacity {
      let discard = self.events.len() - self.capacity;
      self.events.drain(0..discard);
    }
  }

  /// Returns a cloned snapshot of buffered events.
  #[must_use]
  pub fn snapshot(&self) -> Vec<EventStreamEvent<TB>> {
    self.events.clone()
  }

  /// Capacity accessor.
  #[must_use]
  pub const fn capacity(&self) -> usize {
    self.capacity
  }
}

impl<TB: RuntimeToolbox + 'static> Default for EventStreamEventsGeneric<TB> {
  fn default() -> Self {
    Self::with_capacity(DEFAULT_CAPACITY)
  }
}
