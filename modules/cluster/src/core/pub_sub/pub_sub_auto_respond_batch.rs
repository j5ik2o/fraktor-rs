//! Auto-responding batch payload delivered to subscribers.

use alloc::vec::Vec;

use fraktor_actor_core_rs::core::kernel::actor::messaging::AnyMessage;

/// Batch wrapper that carries decoded messages.
pub struct PubSubAutoRespondBatch {
  /// Decoded messages in this batch.
  pub messages: Vec<AnyMessage>,
}
