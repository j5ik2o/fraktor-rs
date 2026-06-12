//! Contract deriving the routing target (entity id / shard id) from a message.

use alloc::string::String;

#[cfg(test)]
#[path = "sharding_message_extractor_test.rs"]
mod tests;

/// Pluggable contract deriving the routing target from a message.
///
/// Mirrors Pekko's `ShardingMessageExtractor[E, M]` where `E` is the incoming
/// message type (typically [`ShardingEnvelope`](super::ShardingEnvelope)) and
/// `M` is the inner message type.
///
/// Implementations MUST be pure: the same input always yields the same
/// derivation, independent of host capabilities or node topology.
pub trait ShardingMessageExtractor<E, M>: Send + Sync {
  /// Derives the entity id, or `None` when it cannot be derived.
  fn entity_id(&self, message: &E) -> Option<String>;

  /// Derives the shard id for the given entity id.
  fn shard_id(&self, entity_id: &str) -> String;

  /// Unwraps the inner message.
  fn unwrap_message(&self, message: E) -> M;
}
