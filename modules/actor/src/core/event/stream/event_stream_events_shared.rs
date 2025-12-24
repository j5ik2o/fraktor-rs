//! Event buffer protected by runtime-provided locking.

use alloc::vec::Vec;

use fraktor_utils_rs::core::{
  runtime_toolbox::{RuntimeToolbox, SyncRwLockFamily, ToolboxRwLock},
  sync::{ArcShared, sync_rwlock_like::SyncRwLockLike},
};

use crate::core::event::stream::{EventStreamEvent, EventStreamEventsGeneric};

/// Shared, lock-protected event buffer.
pub struct EventStreamEventsSharedGeneric<TB: RuntimeToolbox + 'static> {
  events: ArcShared<ToolboxRwLock<EventStreamEventsGeneric<TB>, TB>>,
}

impl<TB: RuntimeToolbox + 'static> EventStreamEventsSharedGeneric<TB> {
  /// Creates an empty buffer with the specified capacity.
  #[must_use]
  pub fn with_capacity(capacity: usize) -> Self {
    Self {
      events: ArcShared::new(<TB::RwLockFamily as SyncRwLockFamily>::create(EventStreamEventsGeneric::with_capacity(
        capacity,
      ))),
    }
  }

  /// Pushes an event and trims the buffer if it exceeds capacity.
  pub fn push_and_trim(&self, event: EventStreamEvent<TB>) {
    let mut guard = self.events.write();
    guard.push_and_trim(event);
  }

  /// Returns a cloned snapshot of buffered events.
  #[must_use]
  pub fn snapshot(&self) -> Vec<EventStreamEvent<TB>> {
    let guard = self.events.read();
    guard.snapshot()
  }
}

impl<TB: RuntimeToolbox + 'static> Default for EventStreamEventsSharedGeneric<TB> {
  fn default() -> Self {
    Self::with_capacity(super::event_stream_events::DEFAULT_CAPACITY)
  }
}

impl<TB: RuntimeToolbox + 'static> Clone for EventStreamEventsSharedGeneric<TB> {
  fn clone(&self) -> Self {
    Self { events: self.events.clone() }
  }
}
