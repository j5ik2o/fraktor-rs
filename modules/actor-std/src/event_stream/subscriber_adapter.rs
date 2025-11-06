use cellactor_actor_core_rs::event_stream::EventStreamSubscriber as CoreEventStreamSubscriber;
use cellactor_utils_core_rs::sync::ArcShared;
use cellactor_utils_std_rs::runtime_toolbox::StdToolbox;

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
