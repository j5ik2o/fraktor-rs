//! Message envelope carrying the target entity id alongside the inner message.

use alloc::string::String;

#[cfg(test)]
#[path = "sharding_envelope_test.rs"]
mod tests;

/// Envelope pairing a target entity id with the inner message.
///
/// Mirrors Pekko's `ShardingEnvelope[M]`. The envelope performs no
/// validation; identity validation is owned by
/// [`ClusterIdentity`](crate::activation::ClusterIdentity).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShardingEnvelope<M> {
  entity_id: String,
  message:   M,
}

impl<M> ShardingEnvelope<M> {
  /// Creates a new envelope from an entity id and the inner message.
  #[must_use]
  pub fn new(entity_id: impl Into<String>, message: M) -> Self {
    Self { entity_id: entity_id.into(), message }
  }

  /// Returns the entity id given at construction.
  #[must_use]
  #[allow(clippy::missing_const_for_fn)] // String の Deref が const でないため const fn にできない
  pub fn entity_id(&self) -> &str {
    &self.entity_id
  }

  /// Returns the inner message given at construction.
  #[must_use]
  pub const fn message(&self) -> &M {
    &self.message
  }

  /// Consumes the envelope and returns the inner message.
  #[must_use]
  #[allow(clippy::missing_const_for_fn)] // String フィールドの drop が必要なため const fn にできない
  pub fn into_message(self) -> M {
    self.message
  }
}
