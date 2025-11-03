//! Internal subscriber entry used by the event stream.

use cellactor_utils_core_rs::sync::ArcShared;

use crate::{EventStreamSubscriber, NoStdToolbox, RuntimeToolbox};

/// Maps subscription identifiers to subscriber instances.
pub struct EventStreamSubscriberEntry<TB: RuntimeToolbox = NoStdToolbox> {
  id:         u64,
  subscriber: ArcShared<dyn EventStreamSubscriber<TB>>,
}

impl<TB: RuntimeToolbox> EventStreamSubscriberEntry<TB> {
  /// Creates a new subscriber entry.
  #[must_use]
  pub const fn new(id: u64, subscriber: ArcShared<dyn EventStreamSubscriber<TB>>) -> Self {
    Self { id, subscriber }
  }

  /// Returns the subscription identifier.
  #[must_use]
  pub const fn id(&self) -> u64 {
    self.id
  }

  /// Returns the subscriber handle.
  #[must_use]
  pub fn subscriber(&self) -> ArcShared<dyn EventStreamSubscriber<TB>> {
    self.subscriber.clone()
  }
}

impl<TB: RuntimeToolbox> Clone for EventStreamSubscriberEntry<TB> {
  fn clone(&self) -> Self {
    Self { id: self.id, subscriber: self.subscriber.clone() }
  }
}
