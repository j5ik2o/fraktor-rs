//! Internal subscriber entry used by the event stream.

use cellactor_utils_core_rs::sync::ArcShared;

use crate::event_stream_subscriber::EventStreamSubscriber;

/// Maps subscription identifiers to subscriber instances.
#[derive(Clone)]
pub struct EventStreamSubscriberEntry {
  id:         u64,
  subscriber: ArcShared<dyn EventStreamSubscriber>,
}

impl EventStreamSubscriberEntry {
  #[must_use]
  pub const fn new(id: u64, subscriber: ArcShared<dyn EventStreamSubscriber>) -> Self {
    Self { id, subscriber }
  }

  #[must_use]
  pub const fn id(&self) -> u64 {
    self.id
  }

  #[must_use]
  pub fn subscriber(&self) -> ArcShared<dyn EventStreamSubscriber> {
    self.subscriber.clone()
  }
}
