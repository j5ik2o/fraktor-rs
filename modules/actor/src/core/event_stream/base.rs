//! Runtime event stream supporting buffered fanout.

#[cfg(test)]
mod tests;

use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdToolbox, RuntimeToolbox},
  sync::{ArcShared, sync_mutex_like::SyncMutexLike},
};

use crate::core::{
  actor_prim::actor_ref::ActorRefGeneric,
  event_stream::{
    ActorRefEventStreamSubscriber, EventStreamEvent, EventStreamEventsSharedGeneric,
    EventStreamSubscriberEntriesSharedGeneric, EventStreamSubscriberShared, event_stream_events::DEFAULT_CAPACITY,
    event_stream_subscriber::subscriber_handle, event_stream_subscription::EventStreamSubscriptionGeneric,
  },
};

/// In-memory event bus with replay support for late subscribers.
pub struct EventStreamGeneric<TB: RuntimeToolbox + 'static> {
  subscribers: EventStreamSubscriberEntriesSharedGeneric<TB>,
  events:      EventStreamEventsSharedGeneric<TB>,
}

impl<TB: RuntimeToolbox + 'static> EventStreamGeneric<TB> {
  /// Creates a stream with the specified buffer capacity.
  #[must_use]
  pub fn with_capacity(capacity: usize) -> Self {
    Self {
      subscribers: EventStreamSubscriberEntriesSharedGeneric::new(),
      events:      EventStreamEventsSharedGeneric::with_capacity(capacity),
    }
  }

  /// Appends the subscriber and replays buffered events.
  #[must_use]
  pub fn subscribe_arc(
    stream: &ArcShared<Self>,
    subscriber: &EventStreamSubscriberShared<TB>,
  ) -> EventStreamSubscriptionGeneric<TB> {
    let id = stream.subscribers.add(subscriber.clone());

    let snapshot = stream.events.snapshot();
    for event in snapshot.iter() {
      let mut guard = subscriber.lock();
      guard.on_event(event);
    }

    EventStreamSubscriptionGeneric::new(stream.clone(), id)
  }

  /// Subscribes an ActorRef to this event stream.
  ///
  /// Events will be delivered **asynchronously** to the actor's mailbox.
  /// This is the **recommended way** for actor-based subscribers as it provides:
  /// - Non-blocking `publish()` (immediate return)
  /// - Better scalability with many subscribers
  /// - Natural actor processing model
  #[must_use]
  pub fn subscribe_actor(
    stream: &ArcShared<Self>,
    actor_ref: ActorRefGeneric<TB>,
  ) -> EventStreamSubscriptionGeneric<TB> {
    let subscriber = subscriber_handle(ActorRefEventStreamSubscriber::new(actor_ref));
    Self::subscribe_arc(stream, &subscriber)
  }

  /// Removes the subscriber associated with the identifier.
  pub fn unsubscribe(&self, id: u64) {
    self.subscribers.remove(id);
  }

  /// Publishes the provided event to all registered subscribers.
  pub fn publish(&self, event: &EventStreamEvent<TB>) {
    self.events.push_and_trim(event.clone());

    let subscribers = self.subscribers.snapshot();
    for entry in subscribers.iter() {
      let handle = entry.subscriber();
      let mut guard = handle.lock();
      guard.on_event(event);
    }
  }
}

impl<TB: RuntimeToolbox + 'static> Default for EventStreamGeneric<TB> {
  fn default() -> Self {
    Self::with_capacity(DEFAULT_CAPACITY)
  }
}

/// Type alias for `EventStreamGeneric` with the default `NoStdToolbox`.
pub type EventStream = EventStreamGeneric<NoStdToolbox>;
