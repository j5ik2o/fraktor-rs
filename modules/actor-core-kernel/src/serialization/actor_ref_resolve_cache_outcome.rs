//! Cache lookup outcome for actor reference resolution.

/// Result of resolving an actor path through
/// [`crate::serialization::ActorRefResolveCache`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ActorRefResolveCacheOutcome<V> {
  /// The value was returned from the cache.
  Hit(V),
  /// The value was computed by the resolver.
  Miss(V),
}
