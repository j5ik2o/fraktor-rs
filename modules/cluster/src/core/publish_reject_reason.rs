//! Publish rejection reasons.

/// Reason describing why a publish request was rejected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PublishRejectReason {
  /// Topic is invalid (empty or malformed).
  InvalidTopic,
  /// Payload is invalid.
  InvalidPayload,
  /// Payload cannot be serialized.
  NotSerializable,
  /// Queue is full and cannot accept more messages.
  QueueFull,
  /// No subscribers are registered for the topic.
  NoSubscribers,
  /// Publish dropped due to partition handling policy.
  PartitionDrop,
}
