//! Configuration for the work-pulling producer controller.

use core::time::Duration;

use crate::delivery::ProducerControllerConfig;

#[cfg(test)]
#[path = "work_pulling_producer_controller_config_test.rs"]
mod tests;

/// Default buffer size for buffered messages awaiting worker demand.
const DEFAULT_BUFFER_SIZE: u32 = 1000;

/// Default internal ask timeout.
const DEFAULT_INTERNAL_ASK_TIMEOUT: Duration = Duration::from_secs(60);

/// Configuration for
/// [`WorkPullingProducerController`](super::WorkPullingProducerController).
///
/// Corresponds to Pekko's `WorkPullingProducerController.Settings`.
#[derive(Debug, Clone)]
pub struct WorkPullingProducerControllerConfig {
  buffer_size:                  u32,
  internal_ask_timeout:         Duration,
  producer_controller_settings: ProducerControllerConfig,
}

impl WorkPullingProducerControllerConfig {
  /// Creates default config.
  #[must_use]
  pub const fn new() -> Self {
    Self {
      buffer_size:                  DEFAULT_BUFFER_SIZE,
      internal_ask_timeout:         DEFAULT_INTERNAL_ASK_TIMEOUT,
      producer_controller_settings: ProducerControllerConfig::new(),
    }
  }

  /// Returns the maximum number of messages buffered while waiting for worker
  /// demand.
  #[must_use]
  pub const fn buffer_size(&self) -> u32 {
    self.buffer_size
  }

  /// Returns a new config with the given buffer size.
  ///
  /// Corresponds to Pekko's `WorkPullingProducerController.Settings.withBufferSize`.
  #[must_use]
  pub const fn with_buffer_size(self, size: u32) -> Self {
    Self { buffer_size: size, ..self }
  }

  /// Returns the internal ask timeout used for protocol-internal messages.
  ///
  /// Corresponds to Pekko's `WorkPullingProducerController.Settings.internalAskTimeout`.
  #[must_use]
  pub const fn internal_ask_timeout(&self) -> Duration {
    self.internal_ask_timeout
  }

  /// Returns a new config with the given internal ask timeout.
  ///
  /// Corresponds to Pekko's `WorkPullingProducerController.Settings.withInternalAskTimeout`.
  #[must_use]
  pub const fn with_internal_ask_timeout(self, timeout: Duration) -> Self {
    Self { internal_ask_timeout: timeout, ..self }
  }

  /// Returns the nested producer-controller config applied to spawned worker
  /// controllers and durable-queue retries.
  #[must_use]
  pub const fn producer_controller_settings(&self) -> &ProducerControllerConfig {
    &self.producer_controller_settings
  }

  /// Returns a new config with the given nested producer-controller config.
  #[must_use]
  pub const fn with_producer_controller_settings(self, settings: ProducerControllerConfig) -> Self {
    Self { producer_controller_settings: settings, ..self }
  }
}

impl Default for WorkPullingProducerControllerConfig {
  fn default() -> Self {
    Self::new()
  }
}
