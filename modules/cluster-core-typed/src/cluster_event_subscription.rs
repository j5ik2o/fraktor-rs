//! Typed cluster event subscription handle.

use fraktor_actor_core_kernel_rs::event::stream::EventStreamSubscription;
use fraktor_utils_core_rs::sync::{SharedAccess, SharedLock};

/// Subscription handle for typed cluster event delivery.
pub struct ClusterEventSubscription {
  subscription:      EventStreamSubscription,
  failed_deliveries: SharedLock<u64>,
}

impl ClusterEventSubscription {
  pub(crate) const fn new(subscription: EventStreamSubscription, failed_deliveries: SharedLock<u64>) -> Self {
    Self { subscription, failed_deliveries }
  }

  /// Returns the kernel event-stream subscription identifier.
  #[must_use]
  pub const fn id(&self) -> u64 {
    self.subscription.id()
  }

  /// Returns the number of typed messages that could not be delivered.
  #[must_use]
  pub fn failed_delivery_count(&self) -> u64 {
    self.failed_deliveries.with_read(|count| *count)
  }
}
