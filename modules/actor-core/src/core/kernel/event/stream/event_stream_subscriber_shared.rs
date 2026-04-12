//! Shared wrapper for event stream subscribers.

use alloc::boxed::Box;

use fraktor_utils_core_rs::core::sync::SharedLock;

use crate::core::kernel::event::stream::{EventStreamEvent, EventStreamSubscriber};

/// Shared wrapper that serializes access to an event stream subscriber.
pub struct EventStreamSubscriberShared {
  inner: SharedLock<Box<dyn EventStreamSubscriber>>,
}

impl EventStreamSubscriberShared {
  /// Creates a shared wrapper from an already materialized shared lock.
  #[must_use]
  pub const fn from_shared_lock(inner: SharedLock<Box<dyn EventStreamSubscriber>>) -> Self {
    Self { inner }
  }

  /// Delivers an event to the wrapped subscriber under the subscriber lock.
  pub fn notify(&self, event: &EventStreamEvent) {
    self.inner.with_lock(|subscriber| subscriber.on_event(event));
  }
}

impl Clone for EventStreamSubscriberShared {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}
