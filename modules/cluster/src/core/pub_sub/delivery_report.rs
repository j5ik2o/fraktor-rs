//! Aggregated delivery report.

use alloc::vec::Vec;

use fraktor_utils_rs::core::runtime_toolbox::RuntimeToolbox;

use super::{DeliveryStatus, SubscriberDeliveryReport};

/// Aggregated delivery result for a batch.
pub struct DeliveryReport<TB: RuntimeToolbox> {
  /// Overall status.
  pub status: DeliveryStatus,
  /// Failed subscribers.
  pub failed: Vec<SubscriberDeliveryReport<TB>>,
}
