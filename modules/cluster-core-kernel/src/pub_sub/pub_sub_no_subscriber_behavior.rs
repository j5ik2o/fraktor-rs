//! Behavior used when distributed pub-sub has no matching target.

/// Outcome strategy when publish or path delivery finds no target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PubSubNoSubscriberBehavior {
  /// Drop the message without creating a delivery intent.
  #[default]
  Drop,
  /// Emit a dead-letter delivery intent.
  DeadLetter,
}
