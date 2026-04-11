//! Shared wrapper for [`EventStream`] with deadlock-safe callback execution.
//!
//! This module provides thread-safe access to [`EventStream`] while ensuring
//! that subscriber callbacks are executed without holding the event stream lock,
//! preventing potential deadlocks.

use fraktor_utils_core_rs::core::sync::{SharedAccess, SharedRwLock, SpinSyncRwLock};

use crate::core::kernel::{
  actor::actor_ref::ActorRef,
  event::stream::{
    ActorRefEventStreamSubscriber, EventStream, EventStreamEvent, EventStreamSubscriberShared,
    event_stream_events::DEFAULT_CAPACITY, event_stream_subscriber::subscriber_handle,
    event_stream_subscription::EventStreamSubscription,
  },
};

/// Shared wrapper that provides thread-safe access to [`EventStream`].
///
/// This type implements the hybrid locking pattern to avoid deadlocks:
/// - Lock acquisition is minimized to data mutation only
/// - Subscriber callbacks are executed after releasing the lock
///
/// # Design
///
/// The key insight is separating "data mutation" from "callback execution":
///
/// ```text
/// publish():
///   1. Acquire write lock
///   2. Store event, get subscriber snapshot
///   3. Release lock  <-- Lock released BEFORE callbacks
///   4. Notify each subscriber (no lock held)
/// ```
///
/// This prevents deadlocks where:
/// - Thread A holds EventStream lock and calls subscriber callback
/// - Callback tries to access EventStream (directly or indirectly)
/// - Thread B holds a lock that Thread A's callback needs
/// - Thread B tries to access EventStream → deadlock
pub struct EventStreamShared {
  inner: SharedRwLock<EventStream>,
}

impl EventStreamShared {
  /// Creates a shared event stream with the specified buffer capacity.
  #[must_use]
  pub fn with_capacity(capacity: usize) -> Self {
    Self { inner: SharedRwLock::new_with_driver::<SpinSyncRwLock<_>>(EventStream::with_capacity(capacity)) }
  }

  /// Subscribes and replays buffered events to the subscriber.
  ///
  /// Events are replayed after releasing the lock to prevent deadlocks.
  #[must_use]
  pub fn subscribe(&self, subscriber: &EventStreamSubscriberShared) -> EventStreamSubscription {
    // Phase 1: Acquire lock, register subscriber, get replay snapshot
    let (id, snapshot) = self.inner.with_write(|guard| guard.subscribe(subscriber.clone()));
    // Lock released here!

    // Phase 2: Replay buffered events without holding the event stream lock.
    // Each callback runs under the subscriber's own lock so that
    // `DebugSpinSyncMutex`-instrumented overrides can detect re-entrant
    // subscriber acquisitions.
    for event in snapshot.iter() {
      subscriber.with_lock(|guard| guard.on_event(event));
    }

    EventStreamSubscription::new(self.clone(), id)
  }

  /// Subscribes without replaying buffered events.
  #[must_use]
  pub fn subscribe_no_replay(&self, subscriber: &EventStreamSubscriberShared) -> EventStreamSubscription {
    let id = self.inner.with_write(|guard| guard.subscribe_no_replay(subscriber.clone()));
    EventStreamSubscription::new(self.clone(), id)
  }

  /// Subscribes an ActorRef to this event stream.
  ///
  /// Events will be delivered **asynchronously** to the actor's mailbox.
  /// This is the **recommended way** for actor-based subscribers as it provides:
  /// - Non-blocking `publish()` (immediate return)
  /// - Better scalability with many subscribers
  /// - Natural actor processing model
  #[must_use]
  pub fn subscribe_actor(&self, actor_ref: ActorRef) -> EventStreamSubscription {
    let subscriber = subscriber_handle(ActorRefEventStreamSubscriber::new(actor_ref));
    self.subscribe(&subscriber)
  }

  /// Removes the subscriber associated with the identifier.
  pub fn unsubscribe(&self, id: u64) {
    self.inner.with_write(|guard| guard.unsubscribe(id));
  }

  /// Publishes the provided event to all registered subscribers.
  ///
  /// Subscribers are notified after releasing the lock to prevent deadlocks.
  pub fn publish(&self, event: &EventStreamEvent) {
    // Phase 1: Acquire lock, store event, get subscriber snapshot
    let subscribers = self.inner.with_write(|guard| guard.publish_prepare(event.clone()));
    // Lock released here!

    // Phase 2: Notify subscribers without holding the event stream lock.
    // Each callback runs under the subscriber's own lock so that
    // `DebugSpinSyncMutex`-instrumented overrides can detect re-entrant
    // subscriber acquisitions.
    for entry in subscribers.iter() {
      let handle = entry.subscriber();
      handle.with_lock(|guard| guard.on_event(event));
    }
  }
}

impl Default for EventStreamShared {
  fn default() -> Self {
    Self::with_capacity(DEFAULT_CAPACITY)
  }
}

impl Clone for EventStreamShared {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}

impl PartialEq for EventStreamShared {
  fn eq(&self, other: &Self) -> bool {
    SharedRwLock::ptr_eq(&self.inner, &other.inner)
  }
}

impl Eq for EventStreamShared {}

impl SharedAccess<EventStream> for EventStreamShared {
  fn with_read<R>(&self, f: impl FnOnce(&EventStream) -> R) -> R {
    self.inner.with_read(f)
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut EventStream) -> R) -> R {
    self.inner.with_write(f)
  }
}
