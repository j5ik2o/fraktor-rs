use fraktor_actor_core_rs::core::event_stream::EventStreamSubscriber as CoreEventStreamSubscriber;
use fraktor_utils_core_rs::{core::sync::ArcShared, std::runtime_toolbox::StdToolbox};

use super::{EventStreamEvent, EventStreamSubscriber};

pub(crate) struct EventStreamSubscriberAdapter {
  inner: ArcShared<dyn EventStreamSubscriber>,
}

impl EventStreamSubscriberAdapter {
  pub(crate) const fn new(inner: ArcShared<dyn EventStreamSubscriber>) -> Self {
    Self { inner }
  }
}

impl CoreEventStreamSubscriber<StdToolbox> for EventStreamSubscriberAdapter {
  fn on_event(&self, event: &EventStreamEvent) {
    self.inner.on_event(event);
  }
}
