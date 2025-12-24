//! Shared wrapper for [`EventStreamGeneric`] with deadlock-safe callback execution.
//!
//! This module provides thread-safe access to [`EventStreamGeneric`] while ensuring
//! that subscriber callbacks are executed without holding the event stream lock,
//! preventing potential deadlocks.

use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdToolbox, RuntimeToolbox, SyncRwLockFamily, ToolboxRwLock},
  sync::{ArcShared, SharedAccess, sync_mutex_like::SyncMutexLike, sync_rwlock_like::SyncRwLockLike},
};

use crate::core::{
  actor_prim::actor_ref::ActorRefGeneric,
  event::stream::{
    ActorRefEventStreamSubscriber, EventStreamEvent, EventStreamGeneric, EventStreamSubscriberShared,
    event_stream_events::DEFAULT_CAPACITY, event_stream_subscriber::subscriber_handle,
    event_stream_subscription::EventStreamSubscriptionGeneric,
  },
};

/// Shared wrapper that provides thread-safe access to [`EventStreamGeneric`].
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
pub struct EventStreamSharedGeneric<TB: RuntimeToolbox + 'static> {
  inner: ArcShared<ToolboxRwLock<EventStreamGeneric<TB>, TB>>,
}

impl<TB: RuntimeToolbox + 'static> EventStreamSharedGeneric<TB> {
  /// Creates a shared event stream with the specified buffer capacity.
  #[must_use]
  pub fn with_capacity(capacity: usize) -> Self {
    Self {
      inner: ArcShared::new(<TB::RwLockFamily as SyncRwLockFamily>::create(EventStreamGeneric::with_capacity(
        capacity,
      ))),
    }
  }

  /// Subscribes and replays buffered events to the subscriber.
  ///
  /// Events are replayed after releasing the lock to prevent deadlocks.
  #[must_use]
  pub fn subscribe(&self, subscriber: &EventStreamSubscriberShared<TB>) -> EventStreamSubscriptionGeneric<TB> {
    // Phase 1: Acquire lock, register subscriber, get replay snapshot
    let (id, snapshot) = {
      let mut guard = self.inner.write();
      guard.subscribe(subscriber.clone())
    };
    // Lock released here!

    // Phase 2: Replay buffered events without holding the lock
    for event in snapshot.iter() {
      let mut guard = subscriber.lock();
      guard.on_event(event);
    }

    EventStreamSubscriptionGeneric::new(self.clone(), id)
  }

  /// Subscribes an ActorRef to this event stream.
  ///
  /// Events will be delivered **asynchronously** to the actor's mailbox.
  /// This is the **recommended way** for actor-based subscribers as it provides:
  /// - Non-blocking `publish()` (immediate return)
  /// - Better scalability with many subscribers
  /// - Natural actor processing model
  #[must_use]
  pub fn subscribe_actor(&self, actor_ref: ActorRefGeneric<TB>) -> EventStreamSubscriptionGeneric<TB> {
    let subscriber = subscriber_handle(ActorRefEventStreamSubscriber::new(actor_ref));
    self.subscribe(&subscriber)
  }

  /// Removes the subscriber associated with the identifier.
  pub fn unsubscribe(&self, id: u64) {
    let mut guard = self.inner.write();
    guard.unsubscribe(id);
  }

  /// Publishes the provided event to all registered subscribers.
  ///
  /// Subscribers are notified after releasing the lock to prevent deadlocks.
  pub fn publish(&self, event: &EventStreamEvent<TB>) {
    // Phase 1: Acquire lock, store event, get subscriber snapshot
    let subscribers = {
      let mut guard = self.inner.write();
      guard.publish_prepare(event.clone())
    };
    // Lock released here!

    // Phase 2: Notify subscribers without holding the lock
    for entry in subscribers.iter() {
      let handle = entry.subscriber();
      let mut guard = handle.lock();
      guard.on_event(event);
    }
  }
}

impl<TB: RuntimeToolbox + 'static> Default for EventStreamSharedGeneric<TB> {
  fn default() -> Self {
    Self::with_capacity(DEFAULT_CAPACITY)
  }
}

impl<TB: RuntimeToolbox + 'static> Clone for EventStreamSharedGeneric<TB> {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}

impl<TB: RuntimeToolbox + 'static> PartialEq for EventStreamSharedGeneric<TB> {
  fn eq(&self, other: &Self) -> bool {
    ArcShared::ptr_eq(&self.inner, &other.inner)
  }
}

impl<TB: RuntimeToolbox + 'static> Eq for EventStreamSharedGeneric<TB> {}

impl<TB: RuntimeToolbox + 'static> SharedAccess<EventStreamGeneric<TB>> for EventStreamSharedGeneric<TB> {
  fn with_read<R>(&self, f: impl FnOnce(&EventStreamGeneric<TB>) -> R) -> R {
    let guard = self.inner.read();
    f(&guard)
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut EventStreamGeneric<TB>) -> R) -> R {
    let mut guard = self.inner.write();
    f(&mut guard)
  }
}

/// Type alias for `EventStreamSharedGeneric` with the default `NoStdToolbox`.
pub type EventStreamShared = EventStreamSharedGeneric<NoStdToolbox>;
