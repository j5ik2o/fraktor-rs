//! Pub/Sub configuration shared across core and std layers.

use core::time::Duration;

/// Runtime configuration for pub/sub delivery.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PubSubConfig {
  /// Timeout used when delivering to subscribers.
  pub subscriber_timeout: Duration,
  /// TTL for suspended subscribers.
  pub suspended_ttl:      Duration,
}

impl PubSubConfig {
  /// Creates a configuration with explicit values.
  #[must_use]
  pub const fn new(subscriber_timeout: Duration, suspended_ttl: Duration) -> Self {
    Self { subscriber_timeout, suspended_ttl }
  }
}

impl Default for PubSubConfig {
  fn default() -> Self {
    Self { subscriber_timeout: Duration::from_secs(3), suspended_ttl: Duration::from_secs(60) }
  }
}
