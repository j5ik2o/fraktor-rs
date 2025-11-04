use cellactor_actor_core_rs::eventstream::EventStreamSubscriber as CoreEventStreamSubscriber;
use cellactor_utils_core_rs::sync::ArcShared;
use cellactor_utils_std_rs::StdToolbox;

use super::{EventStreamEvent, EventStreamSubscriber};

pub(super) struct EventStreamSubscriberAdapter {
  inner: ArcShared<dyn EventStreamSubscriber>,
}

impl EventStreamSubscriberAdapter {
  pub(super) const fn new(inner: ArcShared<dyn EventStreamSubscriber>) -> Self {
    Self { inner }
  }
}

impl CoreEventStreamSubscriber<StdToolbox> for EventStreamSubscriberAdapter {
  fn on_event(&self, event: &EventStreamEvent) {
    self.inner.on_event(event);
  }
}
