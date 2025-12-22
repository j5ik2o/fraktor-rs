//! Delivery endpoint trait for pub/sub.

use fraktor_utils_rs::core::runtime_toolbox::RuntimeToolbox;

use crate::core::{DeliverBatchRequest, DeliveryReport, PubSubError};

/// Delivery interface implemented by core/std adapters.
pub trait DeliveryEndpoint<TB: RuntimeToolbox>: Send + Sync {
  /// Delivers a batch to the provided subscribers.
  ///
  /// # Errors
  ///
  /// Returns `PubSubError` only for system-level failures.
  fn deliver(&mut self, request: DeliverBatchRequest<TB>) -> Result<DeliveryReport<TB>, PubSubError>;
}
