//! Persistence subsystem settings.

use crate::core::at_least_once_delivery_config::AtLeastOnceDeliveryConfig;

/// Settings shared by the persistence extension.
#[derive(Clone, Debug, Default)]
pub struct PersistenceSettings {
  at_least_once_delivery: AtLeastOnceDeliveryConfig,
}

impl PersistenceSettings {
  /// Creates settings with explicit at-least-once delivery configuration.
  #[must_use]
  pub const fn new(at_least_once_delivery: AtLeastOnceDeliveryConfig) -> Self {
    Self { at_least_once_delivery }
  }

  /// Returns the at-least-once delivery configuration.
  #[must_use]
  pub const fn at_least_once_delivery(&self) -> &AtLeastOnceDeliveryConfig {
    &self.at_least_once_delivery
  }
}
