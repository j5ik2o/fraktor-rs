//! Subscription handle managing event stream registrations.

#[cfg(test)]
mod tests;

use fraktor_utils_core_rs::core::{
  runtime_toolbox::{NoStdToolbox, RuntimeToolbox},
  sync::ArcShared,
};

use crate::core::event_stream::EventStreamGeneric;

/// RAII wrapper ensuring subscribers are removed when dropped.
pub struct EventStreamSubscriptionGeneric<TB: RuntimeToolbox + 'static> {
  stream: ArcShared<EventStreamGeneric<TB>>,
  id:     u64,
}

impl<TB: RuntimeToolbox + 'static> EventStreamSubscriptionGeneric<TB> {
  /// Creates a new subscription handle.
  #[must_use]
  pub const fn new(stream: ArcShared<EventStreamGeneric<TB>>, id: u64) -> Self {
    Self { stream, id }
  }

  /// Returns the unique subscription identifier.
  #[must_use]
  pub const fn id(&self) -> u64 {
    self.id
  }
}

impl<TB: RuntimeToolbox + 'static> Drop for EventStreamSubscriptionGeneric<TB> {
  fn drop(&mut self) {
    self.stream.unsubscribe(self.id);
  }
}

/// Type alias for `EventStreamSubscriptionGeneric` with the default `NoStdToolbox`.
pub type EventStreamSubscription = EventStreamSubscriptionGeneric<NoStdToolbox>;
