//! Subscription handle managing event stream registrations.

#[cfg(test)]
mod tests;

use cellactor_utils_core_rs::sync::ArcShared;

use crate::{NoStdToolbox, RuntimeToolbox, eventstream::EventStreamGeneric};

/// RAII wrapper ensuring subscribers are removed when dropped.
pub struct EventStreamSubscriptionGeneric<TB: RuntimeToolbox + 'static = NoStdToolbox> {
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

/// Type alias for EventStreamSubscription using the default toolbox.
pub type EventStreamSubscription = EventStreamSubscriptionGeneric<NoStdToolbox>;
