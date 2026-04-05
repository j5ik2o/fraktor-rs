use alloc::vec::Vec;

use fraktor_actor_rs::core::kernel::event::stream::{EventStreamEvent, EventStreamSubscriber};
use fraktor_utils_rs::core::sync::{ArcShared, NoStdMutex};

pub(crate) struct RecordingSubscriber {
  events: ArcShared<NoStdMutex<Vec<EventStreamEvent>>>,
}

impl RecordingSubscriber {
  pub(crate) fn new(events: ArcShared<NoStdMutex<Vec<EventStreamEvent>>>) -> Self {
    Self { events }
  }
}

impl EventStreamSubscriber for RecordingSubscriber {
  fn on_event(&mut self, event: &EventStreamEvent) {
    self.events.lock().push(event.clone());
  }
}
