//! Per-subscriber delivery result.

use fraktor_utils_rs::core::runtime_toolbox::RuntimeToolbox;

use super::{DeliveryStatus, PubSubSubscriber};

/// Delivery report for a single subscriber.
pub struct SubscriberDeliveryReport<TB: RuntimeToolbox> {
  /// Subscriber identifier.
  pub subscriber: PubSubSubscriber<TB>,
  /// Delivery outcome.
  pub status:     DeliveryStatus,
}
