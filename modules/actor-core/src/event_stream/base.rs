//! Runtime event stream supporting buffered fanout.

#[cfg(test)]
mod tests;

use alloc::vec::Vec;
use core::sync::atomic::Ordering;

use fraktor_utils_core_rs::core::{
  runtime_toolbox::SyncMutexFamily,
  sync::{ArcShared, sync_mutex_like::SyncMutexLike},
};
use portable_atomic::AtomicU64;

use crate::{
  NoStdToolbox, RuntimeToolbox, ToolboxMutex,
  event_stream::{
    EventStreamSubscriber, event_stream_event::EventStreamEvent,
    event_stream_subscriber_entry::EventStreamSubscriberEntryGeneric,
    event_stream_subscription::EventStreamSubscriptionGeneric,
  },
};

const DEFAULT_CAPACITY: usize = 256;

/// In-memory event bus with replay support for late subscribers.
pub struct EventStreamGeneric<TB: RuntimeToolbox + 'static> {
  subscribers: ToolboxMutex<Vec<EventStreamSubscriberEntryGeneric<TB>>, TB>,
  buffer:      ToolboxMutex<Vec<EventStreamEvent<TB>>, TB>,
  capacity:    usize,
  next_id:     AtomicU64,
}

impl<TB: RuntimeToolbox + 'static> EventStreamGeneric<TB> {
  /// Creates a stream with the specified buffer capacity.
  #[must_use]
  pub fn with_capacity(capacity: usize) -> Self {
    Self {
      subscribers: <TB::MutexFamily as SyncMutexFamily>::create(Vec::new()),
      buffer: <TB::MutexFamily as SyncMutexFamily>::create(Vec::new()),
      capacity,
      next_id: AtomicU64::new(1),
    }
  }

  /// Appends the subscriber and replays buffered events.
  #[must_use]
  pub fn subscribe_arc(
    stream: &ArcShared<Self>,
    subscriber: &ArcShared<dyn EventStreamSubscriber<TB>>,
  ) -> EventStreamSubscriptionGeneric<TB> {
    let id = stream.next_id.fetch_add(1, Ordering::Relaxed);
    {
      let mut list = stream.subscribers.lock();
      list.push(EventStreamSubscriberEntryGeneric::new(id, subscriber.clone()));
    }

    let snapshot = stream.buffer.lock().clone();
    for event in snapshot.iter() {
      subscriber.on_event(event);
    }

    EventStreamSubscriptionGeneric::new(stream.clone(), id)
  }

  /// Removes the subscriber associated with the identifier.
  pub fn unsubscribe(&self, id: u64) {
    let mut list = self.subscribers.lock();
    if let Some(position) = list.iter().position(|entry| entry.id() == id) {
      list.swap_remove(position);
    }
  }

  /// Publishes the provided event to all registered subscribers.
  pub fn publish(&self, event: &EventStreamEvent<TB>) {
    {
      let mut buffer = self.buffer.lock();
      buffer.push(event.clone());
      if buffer.len() > self.capacity {
        let discard = buffer.len() - self.capacity;
        buffer.drain(0..discard);
      }
    }

    let subscribers = self.subscribers.lock().clone();
    for entry in subscribers.iter() {
      entry.subscriber().on_event(event);
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
