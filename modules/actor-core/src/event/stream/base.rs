//! Runtime event stream supporting buffered fanout.

#[cfg(test)]
mod tests;

use alloc::vec::Vec;

use crate::event::stream::{
  ClassifierKey, EventStreamEvent, EventStreamEvents, EventStreamSubscriberEntries, EventStreamSubscriberEntry,
  EventStreamSubscriberShared, event_stream_events::DEFAULT_CAPACITY,
};

/// In-memory event bus with replay support for late subscribers.
///
/// This type uses `&mut self` methods for state modification, following the
/// interior mutability guideline. For shared access, use [`EventStreamShared`].
///
/// [`EventStreamShared`]: super::EventStreamShared
pub struct EventStream {
  subscribers: EventStreamSubscriberEntries,
  events:      EventStreamEvents,
}

impl EventStream {
  /// Creates a stream with the specified buffer capacity.
  #[must_use]
  pub const fn with_capacity(capacity: usize) -> Self {
    Self { subscribers: EventStreamSubscriberEntries::new(), events: EventStreamEvents::with_capacity(capacity) }
  }

  /// Adds a subscriber for a specific classifier and returns the assigned
  /// identifier along with a snapshot of buffered events for replay.
  ///
  /// The caller is responsible for replaying the snapshot to the subscriber
  /// after releasing any locks.
  #[must_use]
  pub fn subscribe_with_key(
    &mut self,
    key: ClassifierKey,
    subscriber: EventStreamSubscriberShared,
  ) -> (u64, Vec<EventStreamEvent>) {
    let id = self.subscribers.add_with_key(key, subscriber);
    let snapshot = self.events.snapshot_for_key(key);
    (id, snapshot)
  }

  /// Adds a subscriber for all event variants and returns the assigned
  /// identifier along with a snapshot of buffered events for replay.
  ///
  /// This is sugar for [`Self::subscribe_with_key`] with [`ClassifierKey::All`].
  #[must_use]
  pub fn subscribe(&mut self, subscriber: EventStreamSubscriberShared) -> (u64, Vec<EventStreamEvent>) {
    self.subscribe_with_key(ClassifierKey::All, subscriber)
  }

  /// Adds a subscriber without replaying buffered events.
  #[must_use]
  pub fn subscribe_no_replay(&mut self, subscriber: EventStreamSubscriberShared) -> u64 {
    self.subscribers.add(subscriber)
  }

  /// Removes the subscriber associated with the identifier.
  pub fn unsubscribe(&mut self, id: u64) {
    self.subscribers.remove(id);
  }

  /// Stores the event and returns a snapshot of subscribers for notification.
  ///
  /// The caller is responsible for notifying subscribers after releasing any locks.
  /// This separation prevents deadlocks by ensuring callbacks are executed without
  /// holding the event stream lock.
  #[must_use]
  pub fn publish_prepare(&mut self, event: EventStreamEvent) -> Vec<EventStreamSubscriberEntry> {
    let key = ClassifierKey::for_event(&event);
    self.events.push_and_trim(event);
    self.subscribers.snapshot_for(key)
  }

  /// Returns the buffer capacity.
  #[must_use]
  pub const fn capacity(&self) -> usize {
    self.events.capacity()
  }
}

impl Default for EventStream {
  fn default() -> Self {
    Self::with_capacity(DEFAULT_CAPACITY)
  }
}
