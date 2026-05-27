//! Event stream publication payload.

#[cfg(test)]
#[path = "published_event_test.rs"]
mod tests;

use alloc::{collections::BTreeSet, string::String};

use crate::PersistenceId;

/// Event payload published after a successful persist.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PublishedEvent<E> {
  persistence_id: PersistenceId,
  sequence_nr:    u64,
  event:          E,
  timestamp:      u64,
  tags:           BTreeSet<String>,
}

impl<E> PublishedEvent<E> {
  /// Creates a published event payload.
  #[must_use]
  pub const fn new(
    persistence_id: PersistenceId,
    sequence_nr: u64,
    event: E,
    timestamp: u64,
    tags: BTreeSet<String>,
  ) -> Self {
    Self { persistence_id, sequence_nr, event, timestamp, tags }
  }

  /// Returns the persistence id that produced the event.
  #[must_use]
  pub const fn persistence_id(&self) -> &PersistenceId {
    &self.persistence_id
  }

  /// Returns the persisted event sequence number.
  #[must_use]
  pub const fn sequence_nr(&self) -> u64 {
    self.sequence_nr
  }

  /// Returns the published event value.
  #[must_use]
  pub const fn event(&self) -> &E {
    &self.event
  }

  /// Returns the event timestamp.
  #[must_use]
  pub const fn timestamp(&self) -> u64 {
    self.timestamp
  }

  /// Returns the event tags.
  #[must_use]
  pub const fn tags(&self) -> &BTreeSet<String> {
    &self.tags
  }

  /// Returns this published event with all tags removed.
  #[must_use]
  pub fn without_tags(self) -> Self {
    Self {
      persistence_id: self.persistence_id,
      sequence_nr:    self.sequence_nr,
      event:          self.event,
      timestamp:      self.timestamp,
      tags:           BTreeSet::new(),
    }
  }
}
