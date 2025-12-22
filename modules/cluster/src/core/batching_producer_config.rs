//! Configuration for batching producer.

use core::time::Duration;

/// Configuration for batching behaviour.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BatchingProducerConfig {
  /// Maximum number of messages per batch.
  pub batch_size:     usize,
  /// Maximum number of queued messages.
  pub max_queue_size: usize,
  /// Maximum wait time before flushing.
  pub max_wait:       Duration,
}

impl BatchingProducerConfig {
  /// Creates a new configuration.
  #[must_use]
  pub const fn new(batch_size: usize, max_queue_size: usize, max_wait: Duration) -> Self {
    Self { batch_size, max_queue_size, max_wait }
  }
}

impl Default for BatchingProducerConfig {
  fn default() -> Self {
    Self { batch_size: 32, max_queue_size: 256, max_wait: Duration::from_millis(250) }
  }
}
