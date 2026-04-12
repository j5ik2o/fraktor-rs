//! Factory contract for [`EventStreamSubscriberShared`](super::EventStreamSubscriberShared).

use alloc::boxed::Box;

use super::{EventStreamSubscriber, EventStreamSubscriberShared};

/// Materializes [`EventStreamSubscriberShared`] instances.
pub trait EventStreamSubscriberSharedFactory: Send + Sync {
  /// Creates a shared event-stream subscriber wrapper.
  fn create(&self, subscriber: Box<dyn EventStreamSubscriber>) -> EventStreamSubscriberShared;
}
