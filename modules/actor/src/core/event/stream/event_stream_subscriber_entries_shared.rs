//! Subscriber entry collection with runtime-provided locking.

use alloc::vec::Vec;

use fraktor_utils_rs::core::{
  runtime_toolbox::{RuntimeToolbox, SyncRwLockFamily, ToolboxRwLock},
  sync::{ArcShared, sync_rwlock_like::SyncRwLockLike},
};

use crate::core::event::stream::{
  EventStreamSubscriberEntriesGeneric, EventStreamSubscriberEntryGeneric, EventStreamSubscriberShared,
};

/// Shared, lock-protected subscriber entry collection.
pub struct EventStreamSubscriberEntriesSharedGeneric<TB: RuntimeToolbox + 'static> {
  entries: ArcShared<ToolboxRwLock<EventStreamSubscriberEntriesGeneric<TB>, TB>>,
}

impl<TB: RuntimeToolbox + 'static> EventStreamSubscriberEntriesSharedGeneric<TB> {
  /// Creates an empty shared collection.
  #[must_use]
  pub fn new() -> Self {
    Self {
      entries: ArcShared::new(<TB::RwLockFamily as SyncRwLockFamily>::create(
        EventStreamSubscriberEntriesGeneric::new(),
      )),
    }
  }

  /// Adds a subscriber and returns the assigned identifier.
  #[must_use]
  pub fn add(&self, subscriber: EventStreamSubscriberShared<TB>) -> u64 {
    let mut guard = self.entries.write();
    guard.add(subscriber)
  }

  /// Removes a subscriber by identifier if it exists.
  pub fn remove(&self, id: u64) {
    let mut guard = self.entries.write();
    guard.remove(id);
  }

  /// Returns a cloned snapshot of the current subscribers.
  #[must_use]
  pub fn snapshot(&self) -> Vec<EventStreamSubscriberEntryGeneric<TB>> {
    let guard = self.entries.read();
    guard.snapshot()
  }
}

impl<TB: RuntimeToolbox + 'static> Default for EventStreamSubscriberEntriesSharedGeneric<TB> {
  fn default() -> Self {
    Self::new()
  }
}

impl<TB: RuntimeToolbox + 'static> Clone for EventStreamSubscriberEntriesSharedGeneric<TB> {
  fn clone(&self) -> Self {
    Self { entries: self.entries.clone() }
  }
}
