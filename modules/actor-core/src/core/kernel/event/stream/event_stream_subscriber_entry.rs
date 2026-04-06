//! Internal subscriber entry used by the event stream.

#[cfg(test)]
mod tests;

use crate::core::kernel::event::stream::EventStreamSubscriberShared;

/// Maps subscription identifiers to subscriber instances.
pub struct EventStreamSubscriberEntry {
  id:         u64,
  subscriber: EventStreamSubscriberShared,
}

impl EventStreamSubscriberEntry {
  /// Creates a new subscriber entry.
  #[must_use]
  pub const fn new(id: u64, subscriber: EventStreamSubscriberShared) -> Self {
    Self { id, subscriber }
  }

  /// Returns the subscription identifier.
  #[must_use]
  pub const fn id(&self) -> u64 {
    self.id
  }

  /// Returns the subscriber handle.
  #[must_use]
  pub fn subscriber(&self) -> EventStreamSubscriberShared {
    self.subscriber.clone()
  }
}

impl Clone for EventStreamSubscriberEntry {
  fn clone(&self) -> Self {
    Self { id: self.id, subscriber: self.subscriber.clone() }
  }
}
