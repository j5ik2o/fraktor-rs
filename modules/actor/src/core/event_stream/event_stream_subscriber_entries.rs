//! Subscriber entry collection without internal locking.

use alloc::vec::Vec;

use fraktor_utils_rs::core::runtime_toolbox::RuntimeToolbox;

use crate::core::event_stream::{EventStreamSubscriberEntryGeneric, EventStreamSubscriberShared};

/// Collection of subscriber entries with local identifier generation.
pub struct EventStreamSubscriberEntriesGeneric<TB: RuntimeToolbox + 'static> {
  entries: Vec<EventStreamSubscriberEntryGeneric<TB>>,
  next_id: u64,
}

impl<TB: RuntimeToolbox + 'static> EventStreamSubscriberEntriesGeneric<TB> {
  /// Creates an empty collection with the initial identifier set to `1`.
  #[must_use]
  pub const fn new() -> Self {
    Self { entries: Vec::new(), next_id: 1 }
  }

  /// Adds a subscriber and returns the assigned identifier.
  pub fn add(&mut self, subscriber: EventStreamSubscriberShared<TB>) -> u64 {
    let id = self.next_id;
    self.next_id += 1;
    self.entries.push(EventStreamSubscriberEntryGeneric::new(id, subscriber));
    id
  }

  /// Removes a subscriber by identifier if it exists.
  pub fn remove(&mut self, id: u64) {
    if let Some(position) = self.entries.iter().position(|entry| entry.id() == id) {
      self.entries.swap_remove(position);
    }
  }

  /// Returns a cloned snapshot of the current subscribers.
  #[must_use]
  pub fn snapshot(&self) -> Vec<EventStreamSubscriberEntryGeneric<TB>> {
    self.entries.clone()
  }
}

impl<TB: RuntimeToolbox + 'static> Default for EventStreamSubscriberEntriesGeneric<TB> {
  fn default() -> Self {
    Self::new()
  }
}
