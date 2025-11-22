//! Delivery guarantees for pub/sub topics.

/// Delivery semantics that a topic must obey.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DeliveryPolicy {
  /// Best-effort delivery without retries.
  AtMostOnce,
  /// Delivery with potential retries (queue during partition, flush later).
  #[default]
  AtLeastOnce,
}
