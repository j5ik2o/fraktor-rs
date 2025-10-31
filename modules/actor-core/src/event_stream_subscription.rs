//! Handle returned to subscribers for managing their registration lifecycle.

use cellactor_utils_core_rs::sync::ArcShared;

use crate::event_stream::EventStream;

/// RAII wrapper ensuring subscribers are removed when dropped.
pub struct EventStreamSubscription {
  stream: ArcShared<EventStream>,
  id:     u64,
}

impl EventStreamSubscription {
  #[must_use]
  /// Creates a new subscription handle.
  pub const fn new(stream: ArcShared<EventStream>, id: u64) -> Self {
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
