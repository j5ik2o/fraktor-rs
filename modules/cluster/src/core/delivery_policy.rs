//! Delivery guarantees for pub/sub topics.

/// Delivery semantics that a topic must obey.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeliveryPolicy {
  /// Best-effort delivery without retries.
  AtMostOnce,
  /// Delivery with potential retries (queue during partition, flush later).
  AtLeastOnce,
}

impl Default for DeliveryPolicy {
  fn default() -> Self {
    Self::AtLeastOnce
  }
}
