use fraktor_utils_rs::std::runtime_toolbox::StdToolbox;

use super::{EventStreamEvent, EventStreamSubscriberShared};
use crate::core::event_stream::EventStreamSubscriber as CoreEventStreamSubscriber;

/// Adapter bridging standard [`EventStreamSubscriber`] to the core runtime trait.
pub struct EventStreamSubscriberAdapter {
  inner: EventStreamSubscriberShared,
}

impl EventStreamSubscriberAdapter {
  /// Creates a new adapter wrapping the given subscriber.
  #[must_use]
  pub const fn new(inner: EventStreamSubscriberShared) -> Self {
    Self { inner }
  }
}

impl CoreEventStreamSubscriber<StdToolbox> for EventStreamSubscriberAdapter {
  fn on_event(&mut self, event: &EventStreamEvent) {
    let mut guard = self.inner.lock();
    guard.on_event(event);
  }
}
