//! Per-subscriber delivery result.

use super::{DeliveryStatus, PubSubSubscriber};

/// Delivery report for a single subscriber.
pub struct SubscriberDeliveryReport {
  /// Subscriber identifier.
  pub subscriber: PubSubSubscriber,
  /// Delivery outcome.
  pub status:     DeliveryStatus,
}
