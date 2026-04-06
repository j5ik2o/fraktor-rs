//! Subscription handle managing event stream registrations.

#[cfg(test)]
mod tests;

use crate::core::kernel::event::stream::EventStreamShared;

/// RAII wrapper ensuring subscribers are removed when dropped.
pub struct EventStreamSubscription {
  stream: EventStreamShared,
  id:     u64,
}

impl EventStreamSubscription {
  /// Creates a new subscription handle.
  #[must_use]
  pub const fn new(stream: EventStreamShared, id: u64) -> Self {
    Self { stream, id }
  }

  /// Returns the unique subscription identifier.
  #[must_use]
  pub const fn id(&self) -> u64 {
    self.id
  }
}

impl Drop for EventStreamSubscription {
  fn drop(&mut self) {
    self.stream.unsubscribe(self.id);
  }
}
