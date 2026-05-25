//! Configuration for the producer controller.

use core::time::Duration;

#[cfg(test)]
#[path = "producer_controller_config_test.rs"]
mod tests;

/// Default timeout for requests to the durable queue.
const DEFAULT_DURABLE_QUEUE_REQUEST_TIMEOUT: Duration = Duration::from_secs(3);
/// Default retry budget for durable queue requests.
const DEFAULT_DURABLE_QUEUE_RETRY_ATTEMPTS: u32 = 10;
/// Default interval for resending the first unconfirmed message.
const DEFAULT_DURABLE_QUEUE_RESEND_FIRST_INTERVAL: Duration = Duration::from_secs(1);

/// Configuration for [`ProducerController`](super::ProducerController).
///
/// Corresponds to Pekko's `ProducerController.Settings`.
#[derive(Debug, Clone)]
pub struct ProducerControllerConfig {
  durable_queue_request_timeout:       Duration,
  durable_queue_retry_attempts:        u32,
  durable_queue_resend_first_interval: Duration,
}

impl ProducerControllerConfig {
  /// Creates default config.
  #[must_use]
  pub const fn new() -> Self {
    Self {
      durable_queue_request_timeout:       DEFAULT_DURABLE_QUEUE_REQUEST_TIMEOUT,
      durable_queue_retry_attempts:        DEFAULT_DURABLE_QUEUE_RETRY_ATTEMPTS,
      durable_queue_resend_first_interval: DEFAULT_DURABLE_QUEUE_RESEND_FIRST_INTERVAL,
    }
  }

  /// Returns the timeout used for durable queue requests.
  #[must_use]
  pub const fn durable_queue_request_timeout(&self) -> Duration {
    self.durable_queue_request_timeout
  }

  /// Returns a new config with the given durable queue request timeout.
  #[must_use]
  pub const fn with_durable_queue_request_timeout(self, timeout: Duration) -> Self {
    Self { durable_queue_request_timeout: timeout, ..self }
  }

  /// Returns the retry budget used for durable queue requests.
  #[must_use]
  pub const fn durable_queue_retry_attempts(&self) -> u32 {
    self.durable_queue_retry_attempts
  }

  /// Returns a new config with the given durable queue retry budget.
  #[must_use]
  pub const fn with_durable_queue_retry_attempts(self, attempts: u32) -> Self {
    Self { durable_queue_retry_attempts: attempts, ..self }
  }

  /// Returns the interval used for resending the first unconfirmed message.
  #[must_use]
  pub const fn durable_queue_resend_first_interval(&self) -> Duration {
    self.durable_queue_resend_first_interval
  }

  /// Returns a new config with the given resend-first interval.
  #[must_use]
  pub const fn with_durable_queue_resend_first_interval(self, interval: Duration) -> Self {
    Self { durable_queue_resend_first_interval: interval, ..self }
  }
}

impl Default for ProducerControllerConfig {
  fn default() -> Self {
    Self::new()
  }
}
