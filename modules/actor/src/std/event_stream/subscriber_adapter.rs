use fraktor_utils_rs::{core::sync::ArcShared, std::runtime_toolbox::StdToolbox};

use super::{EventStreamEvent, EventStreamSubscriber};
use crate::core::event_stream::EventStreamSubscriber as CoreEventStreamSubscriber;

/// Adapter bridging standard [`EventStreamSubscriber`] to the core runtime trait.
pub struct EventStreamSubscriberAdapter {
  inner: ArcShared<dyn EventStreamSubscriber>,
}

impl EventStreamSubscriberAdapter {
  /// Creates a new adapter wrapping the given subscriber.
  #[must_use]
  pub const fn new(inner: ArcShared<dyn EventStreamSubscriber>) -> Self {
    Self { inner }
  }
}

impl CoreEventStreamSubscriber<StdToolbox> for EventStreamSubscriberAdapter {
  fn on_event(&self, event: &EventStreamEvent) {
    self.inner.on_event(event);
  }
}
