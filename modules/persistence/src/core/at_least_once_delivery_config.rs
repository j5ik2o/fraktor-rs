//! Configuration for at-least-once delivery.

use core::time::Duration;

/// Configuration for at-least-once delivery scheduling.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AtLeastOnceDeliveryConfig {
  redeliver_interval:     Duration,
  redelivery_burst_limit: usize,
  max_unconfirmed:        usize,
}

impl AtLeastOnceDeliveryConfig {
  /// Creates a new configuration.
  #[must_use]
  pub const fn new(redeliver_interval: Duration, redelivery_burst_limit: usize, max_unconfirmed: usize) -> Self {
    Self { redeliver_interval, redelivery_burst_limit, max_unconfirmed }
  }

  /// Returns the redelivery interval.
  #[must_use]
  pub const fn redeliver_interval(&self) -> Duration {
    self.redeliver_interval
  }

  /// Returns the maximum number of messages to redeliver per tick.
  #[must_use]
  pub const fn redelivery_burst_limit(&self) -> usize {
    self.redelivery_burst_limit
  }

  /// Returns the maximum number of unconfirmed messages.
  #[must_use]
  pub const fn max_unconfirmed(&self) -> usize {
    self.max_unconfirmed
  }
}

impl Default for AtLeastOnceDeliveryConfig {
  fn default() -> Self {
    Self { redeliver_interval: Duration::from_secs(1), redelivery_burst_limit: 10, max_unconfirmed: 1000 }
  }
}
