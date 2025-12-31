//! Configuration for at-least-once delivery.

#[cfg(test)]
mod tests;

use core::time::Duration;

/// Configuration values for at-least-once delivery.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AtLeastOnceDeliveryConfig {
  redeliver_interval:     Duration,
  max_unconfirmed:        usize,
  redelivery_burst_limit: usize,
}

impl AtLeastOnceDeliveryConfig {
  /// Creates a new configuration.
  #[must_use]
  pub const fn new(redeliver_interval: Duration, max_unconfirmed: usize, redelivery_burst_limit: usize) -> Self {
    Self { redeliver_interval, max_unconfirmed, redelivery_burst_limit }
  }

  /// Returns the redelivery interval.
  #[must_use]
  pub const fn redeliver_interval(&self) -> Duration {
    self.redeliver_interval
  }

  /// Returns the maximum number of unconfirmed deliveries.
  #[must_use]
  pub const fn max_unconfirmed(&self) -> usize {
    self.max_unconfirmed
  }

  /// Returns the redelivery burst limit.
  #[must_use]
  pub const fn redelivery_burst_limit(&self) -> usize {
    self.redelivery_burst_limit
  }
}

impl Default for AtLeastOnceDeliveryConfig {
  fn default() -> Self {
    Self { redeliver_interval: Duration::from_secs(10), max_unconfirmed: 1000, redelivery_burst_limit: 100 }
  }
}
