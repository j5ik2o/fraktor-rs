//! Runtime event stream supporting subscriber fanout with buffering.

use alloc::vec::Vec;
use core::sync::atomic::{AtomicU64, Ordering};

use cellactor_utils_core_rs::sync::{ArcShared, sync_mutex_like::SpinSyncMutex};

use crate::{
  event_stream_event::EventStreamEvent, event_stream_subscriber::EventStreamSubscriber,
  event_stream_subscriber_entry::EventStreamSubscriberEntry, event_stream_subscription::EventStreamSubscription,
};

const DEFAULT_CAPACITY: usize = 256;

/// In-memory event bus with replay support for new subscribers.
pub struct EventStream {
  subscribers: SpinSyncMutex<Vec<EventStreamSubscriberEntry>>,
  buffer:      SpinSyncMutex<Vec<EventStreamEvent>>,
  capacity:    usize,
  next_id:     AtomicU64,
}

impl EventStream {
  /// Creates a new event stream with the specified buffer capacity.
  #[must_use]
  pub const fn with_capacity(capacity: usize) -> Self {
    Self {
      subscribers: SpinSyncMutex::new(Vec::new()),
      buffer: SpinSyncMutex::new(Vec::new()),
      capacity,
      next_id: AtomicU64::new(1),
    }
  }

  /// Subscribes the given observer and replays buffered events.
  #[must_use]
  pub fn subscribe_arc(
    stream: &ArcShared<Self>,
    subscriber: &ArcShared<dyn EventStreamSubscriber>,
  ) -> EventStreamSubscription {
    let id = stream.next_id.fetch_add(1, Ordering::Relaxed);
    {
      let mut list = stream.subscribers.lock();
      list.push(EventStreamSubscriberEntry::new(id, subscriber.clone()));
    }

    let snapshot = stream.buffer.lock().clone();
    for event in snapshot {
      subscriber.on_event(&event);
    }

    EventStreamSubscription::new(stream.clone(), id)
  }

  /// Removes the subscriber associated with the identifier.
  pub fn unsubscribe(&self, id: u64) {
    let mut list = self.subscribers.lock();
    if let Some(position) = list.iter().position(|entry| entry.id() == id) {
      list.swap_remove(position);
    }
  }

  /// Publishes an event to all registered subscribers.
  pub fn publish(&self, event: &EventStreamEvent) {
    {
      let mut buffer = self.buffer.lock();
      buffer.push(event.clone());
      if buffer.len() > self.capacity {
        let discard = buffer.len() - self.capacity;
        buffer.drain(0..discard);
      }
    }

    let subscribers = self.subscribers.lock().clone();
    for entry in subscribers {
      entry.subscriber().on_event(event);
    }
  }
}

impl Default for EventStream {
  fn default() -> Self {
    Self::with_capacity(DEFAULT_CAPACITY)
  }
}
