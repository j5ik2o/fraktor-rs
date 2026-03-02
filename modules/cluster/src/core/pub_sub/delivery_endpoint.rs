//! Delivery endpoint trait for pub/sub.

use super::{DeliverBatchRequest, DeliveryReport, PubSubError};

/// Delivery interface implemented by core/std adapters.
pub trait DeliveryEndpoint: Send + Sync {
  /// Delivers a batch to the provided subscribers.
  ///
  /// # Errors
  ///
  /// Returns `PubSubError` only for system-level failures.
  fn deliver(&mut self, request: DeliverBatchRequest) -> Result<DeliveryReport, PubSubError>;
}
