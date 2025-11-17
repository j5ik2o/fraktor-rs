//! Internal subscriber entry used by the event stream.

#[cfg(test)]
mod tests;

use fraktor_utils_core_rs::core::{
  runtime_toolbox::{NoStdToolbox, RuntimeToolbox},
  sync::ArcShared,
};

use crate::core::event_stream::EventStreamSubscriber;

/// Maps subscription identifiers to subscriber instances.
pub struct EventStreamSubscriberEntryGeneric<TB: RuntimeToolbox> {
  id:         u64,
  subscriber: ArcShared<dyn EventStreamSubscriber<TB>>,
}

impl<TB: RuntimeToolbox> EventStreamSubscriberEntryGeneric<TB> {
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

impl<TB: RuntimeToolbox> Clone for EventStreamSubscriberEntryGeneric<TB> {
  fn clone(&self) -> Self {
    Self { id: self.id, subscriber: self.subscriber.clone() }
  }
}

/// Type alias for `EventStreamSubscriberEntryGeneric` with the default `NoStdToolbox`.
pub type EventStreamSubscriberEntry = EventStreamSubscriberEntryGeneric<NoStdToolbox>;
