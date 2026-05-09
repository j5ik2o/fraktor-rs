//! Per-topic pub/sub metrics snapshot.

/// Aggregated counters per topic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PubSubTopicMetrics {
  /// Messages queued during partition.
  pub delayed_messages:     u64,
  /// Messages dropped due to policy or missing subscribers.
  pub dropped_messages:     u64,
  /// Messages flushed after recovery.
  pub redelivered_messages: u64,
}
