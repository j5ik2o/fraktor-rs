//! Metrics snapshot for pub/sub broker.

/// Aggregated counters for pub/sub operations.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PubSubMetrics {
  /// Number of messages queued due to partition.
  pub delayed_messages:     u64,
  /// Number of messages dropped (partition or missing subscribers).
  pub dropped_messages:     u64,
  /// Number of queued messages that were flushed after recovery.
  pub redelivered_messages: u64,
}

impl PubSubMetrics {
  /// Creates an empty metrics snapshot.
  #[must_use]
  pub const fn new() -> Self {
    Self { delayed_messages: 0, dropped_messages: 0, redelivered_messages: 0 }
  }
}
