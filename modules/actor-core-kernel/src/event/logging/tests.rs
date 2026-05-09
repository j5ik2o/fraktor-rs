#![cfg(test)]

use alloc::vec::Vec;

use fraktor_utils_core_rs::sync::{ArcShared, SpinSyncMutex};

use crate::event::stream::{EventStreamEvent, EventStreamSubscriber};

pub(crate) struct RecordingSubscriber {
  events: ArcShared<SpinSyncMutex<Vec<EventStreamEvent>>>,
}

impl RecordingSubscriber {
  pub(crate) fn new(events: ArcShared<SpinSyncMutex<Vec<EventStreamEvent>>>) -> Self {
    Self { events }
  }
}

impl EventStreamSubscriber for RecordingSubscriber {
  fn on_event(&mut self, event: &EventStreamEvent) {
    self.events.lock().push(event.clone());
  }
}
