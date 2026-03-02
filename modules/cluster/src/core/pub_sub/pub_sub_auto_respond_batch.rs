//! Auto-responding batch payload delivered to subscribers.

use alloc::vec::Vec;

use fraktor_actor_rs::core::messaging::AnyMessage;

/// Batch wrapper that carries decoded messages.
pub struct PubSubAutoRespondBatch {
  /// Decoded messages in this batch.
  pub messages: Vec<AnyMessage>,
}
