//! Runtime event stream supporting buffered fanout.

#[cfg(test)]
mod tests;

use alloc::vec::Vec;

use fraktor_utils_rs::core::runtime_toolbox::{NoStdToolbox, RuntimeToolbox};

use crate::core::event_stream::{
  EventStreamEvent, EventStreamEventsGeneric, EventStreamSubscriberEntriesGeneric, EventStreamSubscriberEntryGeneric,
  EventStreamSubscriberShared, event_stream_events::DEFAULT_CAPACITY,
};

/// In-memory event bus with replay support for late subscribers.
///
/// This type uses `&mut self` methods for state modification, following the
/// interior mutability guideline. For shared access, use [`EventStreamSharedGeneric`].
///
/// [`EventStreamSharedGeneric`]: super::EventStreamSharedGeneric
pub struct EventStreamGeneric<TB: RuntimeToolbox + 'static> {
  subscribers: EventStreamSubscriberEntriesGeneric<TB>,
  events:      EventStreamEventsGeneric<TB>,
}

impl<TB: RuntimeToolbox + 'static> EventStreamGeneric<TB> {
  /// Creates a stream with the specified buffer capacity.
  #[must_use]
  pub const fn with_capacity(capacity: usize) -> Self {
    Self {
      subscribers: EventStreamSubscriberEntriesGeneric::new(),
      events:      EventStreamEventsGeneric::with_capacity(capacity),
    }
  }

  /// Adds a subscriber and returns the assigned identifier along with a
  /// snapshot of buffered events for replay.
  ///
  /// The caller is responsible for replaying the snapshot to the subscriber
  /// after releasing any locks.
  #[must_use]
  pub fn subscribe(&mut self, subscriber: EventStreamSubscriberShared<TB>) -> (u64, Vec<EventStreamEvent<TB>>) {
    let id = self.subscribers.add(subscriber);
    let snapshot = self.events.snapshot();
    (id, snapshot)
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
  pub fn publish_prepare(&mut self, event: EventStreamEvent<TB>) -> Vec<EventStreamSubscriberEntryGeneric<TB>> {
    self.events.push_and_trim(event);
    self.subscribers.snapshot()
  }

  /// Returns the buffer capacity.
  #[must_use]
  pub const fn capacity(&self) -> usize {
    self.events.capacity()
  }
}

impl<TB: RuntimeToolbox + 'static> Default for EventStreamGeneric<TB> {
  fn default() -> Self {
    Self::with_capacity(DEFAULT_CAPACITY)
  }
}

/// Type alias for `EventStreamGeneric` with the default `NoStdToolbox`.
pub type EventStream = EventStreamGeneric<NoStdToolbox>;
