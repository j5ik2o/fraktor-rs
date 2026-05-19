//! Aggregated delivery report.

use alloc::vec::Vec;

use super::{DeliveryStatus, SubscriberDeliveryReport};

/// Aggregated delivery result for a batch.
pub struct DeliveryReport {
  /// Overall status.
  pub status: DeliveryStatus,
  /// Failed subscribers.
  pub failed: Vec<SubscriberDeliveryReport>,
}
