//! Subscriber entry collection without internal locking.

use alloc::vec::Vec;

use crate::event::stream::{ClassifierKey, EventStreamSubscriberEntry, EventStreamSubscriberShared};

/// Collection of subscriber entries with local identifier generation.
///
/// Subscriber counts are expected to remain small. If they grow to hundreds,
/// replace this tagged `Vec` with a keyed `BTreeMap` in a dedicated follow-up
/// change.
pub(crate) struct EventStreamSubscriberEntries {
  entries: Vec<EventStreamSubscriberEntry>,
  next_id: u64,
}

impl EventStreamSubscriberEntries {
  /// Creates an empty collection with the initial identifier set to `1`.
  #[must_use]
  pub(crate) const fn new() -> Self {
    Self { entries: Vec::new(), next_id: 1 }
  }

  /// Adds a subscriber and returns the assigned identifier.
  pub(crate) fn add(&mut self, subscriber: EventStreamSubscriberShared) -> u64 {
    self.add_with_key(ClassifierKey::All, subscriber)
  }

  /// Adds a subscriber with an explicit classifier and returns the assigned identifier.
  pub(crate) fn add_with_key(&mut self, key: ClassifierKey, subscriber: EventStreamSubscriberShared) -> u64 {
    let id = self.next_id;
    self.next_id += 1;
    self.entries.push(EventStreamSubscriberEntry::new(id, key, subscriber));
    id
  }

  /// Removes a subscriber by identifier if it exists.
  pub(crate) fn remove(&mut self, id: u64) {
    if let Some(position) = self.entries.iter().position(|entry| entry.id() == id) {
      self.entries.swap_remove(position);
    }
  }

  /// Returns a cloned snapshot of the current subscribers.
  #[must_use]
  pub(crate) fn snapshot(&self) -> Vec<EventStreamSubscriberEntry> {
    self.entries.clone()
  }

  /// Returns a cloned snapshot filtered by classifier key.
  #[must_use]
  pub(crate) fn snapshot_for(&self, key: ClassifierKey) -> Vec<EventStreamSubscriberEntry> {
    if key == ClassifierKey::All {
      return self.snapshot();
    }

    self.entries.iter().filter(|entry| entry.key() == key || entry.key() == ClassifierKey::All).cloned().collect()
  }
}

impl Default for EventStreamSubscriberEntries {
  fn default() -> Self {
    Self::new()
  }
}
