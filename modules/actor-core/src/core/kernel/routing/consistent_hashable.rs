//! Contract for messages that expose their own consistent-hash key.

/// Marker-style contract for messages whose consistent-hash key can be
/// derived directly from the payload itself.
///
/// Corresponds to Pekko's `org.apache.pekko.routing.ConsistentHashable`
/// marker trait. Implementations are consulted by
/// [`ConsistentHashingRoutingLogic`](super::ConsistentHashingRoutingLogic)
/// before falling back to the configured hash-key mapper.
pub trait ConsistentHashable {
  /// Returns the stable hash key that identifies this message for routing.
  fn consistent_hash_key(&self) -> u64;
}
