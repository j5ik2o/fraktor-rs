//! Auto-responding batch payload delivered to subscribers.

use alloc::vec::Vec;

use fraktor_actor_rs::core::messaging::AnyMessageGeneric;
use fraktor_utils_rs::core::runtime_toolbox::RuntimeToolbox;

/// Batch wrapper that carries decoded messages.
pub struct PubSubAutoRespondBatch<TB: RuntimeToolbox> {
  /// Decoded messages in this batch.
  pub messages: Vec<AnyMessageGeneric<TB>>,
}
