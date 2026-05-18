//! Internal subscriber entry used by the event stream.

#[cfg(test)]
#[path = "event_stream_subscriber_entry_test.rs"]
mod tests;

use crate::event::stream::{ClassifierKey, EventStreamSubscriberShared};

/// Maps subscription identifiers and classifier keys to subscriber instances.
pub struct EventStreamSubscriberEntry {
  id:         u64,
  key:        ClassifierKey,
  subscriber: EventStreamSubscriberShared,
}

impl EventStreamSubscriberEntry {
  /// Creates a new subscriber entry.
  #[must_use]
  pub const fn new(id: u64, key: ClassifierKey, subscriber: EventStreamSubscriberShared) -> Self {
    Self { id, key, subscriber }
  }

  /// Returns the subscription identifier.
  #[must_use]
  pub const fn id(&self) -> u64 {
    self.id
  }

  /// Returns the classifier associated with the subscriber.
  #[must_use]
  pub const fn key(&self) -> ClassifierKey {
    self.key
  }

  /// Returns the subscriber handle.
  #[must_use]
  pub fn subscriber(&self) -> EventStreamSubscriberShared {
    self.subscriber.clone()
  }
}

impl Clone for EventStreamSubscriberEntry {
  fn clone(&self) -> Self {
    Self { id: self.id, key: self.key, subscriber: self.subscriber.clone() }
  }
}
