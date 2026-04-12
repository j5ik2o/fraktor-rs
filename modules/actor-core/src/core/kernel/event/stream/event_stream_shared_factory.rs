//! Factory contract for [`EventStreamShared`](super::EventStreamShared).

use super::{EventStream, EventStreamShared};

/// Materializes [`EventStreamShared`] instances.
pub trait EventStreamSharedFactory: Send + Sync {
  /// Creates a shared event stream wrapper.
  fn create(&self, stream: EventStream) -> EventStreamShared;
}
