//! Pub/Sub batch payload.

use alloc::vec::Vec;

use crate::core::PubSubEnvelope;

/// Batch of serialized envelopes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PubSubBatch {
  /// Envelope collection.
  pub envelopes: Vec<PubSubEnvelope>,
}

impl PubSubBatch {
  /// Creates a new batch from envelopes.
  #[must_use]
  pub const fn new(envelopes: Vec<PubSubEnvelope>) -> Self {
    Self { envelopes }
  }

  /// Returns true when the batch is empty.
  #[must_use]
  pub const fn is_empty(&self) -> bool {
    self.envelopes.is_empty()
  }
}
