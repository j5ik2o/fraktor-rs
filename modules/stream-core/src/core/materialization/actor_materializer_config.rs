use core::time::Duration;

use super::SubscriptionTimeoutSettings;
use crate::core::{SupervisionStrategy, r#impl::fusing::StreamBufferConfig};

#[cfg(test)]
mod tests;

/// Configuration for [`ActorMaterializer`](super::ActorMaterializer).
pub struct ActorMaterializerConfig {
  drive_interval:        Duration,
  buffer_config:         StreamBufferConfig,
  supervision_strategy:  SupervisionStrategy,
  subscription_timeout:  SubscriptionTimeoutSettings,
  debug_logging:         bool,
  output_burst_limit:    usize,
  max_fixed_buffer_size: usize,
}

impl ActorMaterializerConfig {
  /// Creates a new configuration with defaults.
  #[must_use]
  pub fn new() -> Self {
    Self {
      drive_interval:        Duration::from_millis(10),
      buffer_config:         StreamBufferConfig::default(),
      supervision_strategy:  SupervisionStrategy::Stop,
      subscription_timeout:  SubscriptionTimeoutSettings::default(),
      debug_logging:         false,
      output_burst_limit:    1000,
      max_fixed_buffer_size: 1_000_000_000,
    }
  }

  /// Returns the configured drive interval.
  #[must_use]
  pub const fn drive_interval(&self) -> Duration {
    self.drive_interval
  }

  /// Returns the configured buffer settings.
  #[must_use]
  pub const fn buffer_config(&self) -> StreamBufferConfig {
    self.buffer_config
  }

  /// Returns the configured supervision strategy.
  #[must_use]
  pub const fn supervision_strategy(&self) -> SupervisionStrategy {
    self.supervision_strategy
  }

  /// Returns the configured subscription timeout settings.
  #[must_use]
  pub const fn subscription_timeout(&self) -> SubscriptionTimeoutSettings {
    self.subscription_timeout
  }

  /// Returns whether debug logging is enabled.
  #[must_use]
  pub const fn debug_logging(&self) -> bool {
    self.debug_logging
  }

  /// Returns the configured output burst limit.
  #[must_use]
  pub const fn output_burst_limit(&self) -> usize {
    self.output_burst_limit
  }

  /// Returns the configured maximum fixed buffer size.
  #[must_use]
  pub const fn max_fixed_buffer_size(&self) -> usize {
    self.max_fixed_buffer_size
  }

  /// Updates the drive interval.
  #[must_use]
  pub const fn with_drive_interval(mut self, drive_interval: Duration) -> Self {
    self.drive_interval = drive_interval;
    self
  }

  /// Updates the buffer configuration.
  #[must_use]
  pub const fn with_buffer_config(mut self, buffer_config: StreamBufferConfig) -> Self {
    self.buffer_config = buffer_config;
    self
  }

  /// Updates the supervision strategy.
  #[must_use]
  pub const fn with_supervision_strategy(mut self, supervision_strategy: SupervisionStrategy) -> Self {
    self.supervision_strategy = supervision_strategy;
    self
  }

  /// Updates the subscription timeout settings.
  #[must_use]
  pub const fn with_subscription_timeout(mut self, subscription_timeout: SubscriptionTimeoutSettings) -> Self {
    self.subscription_timeout = subscription_timeout;
    self
  }

  /// Updates whether debug logging is enabled.
  #[must_use]
  pub const fn with_debug_logging(mut self, debug_logging: bool) -> Self {
    self.debug_logging = debug_logging;
    self
  }

  /// Updates the output burst limit.
  #[must_use]
  pub const fn with_output_burst_limit(mut self, output_burst_limit: usize) -> Self {
    self.output_burst_limit = output_burst_limit;
    self
  }

  /// Updates the maximum fixed buffer size.
  #[must_use]
  pub const fn with_max_fixed_buffer_size(mut self, max_fixed_buffer_size: usize) -> Self {
    self.max_fixed_buffer_size = max_fixed_buffer_size;
    self
  }
}

impl Default for ActorMaterializerConfig {
  fn default() -> Self {
    Self::new()
  }
}
