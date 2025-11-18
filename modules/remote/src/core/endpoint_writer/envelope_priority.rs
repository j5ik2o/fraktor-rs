//! Envelope priority enumeration.

/// Differentiates system vs user envelopes.
pub enum EnvelopePriority {
  /// System messages have higher priority.
  System,
  /// User messages are processed after system queues drain.
  User,
}
