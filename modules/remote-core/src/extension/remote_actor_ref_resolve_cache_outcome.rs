//! Observable actor ref resolve cache outcome.

/// Cache outcome exposed through the remote extension event stream.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RemoteActorRefResolveCacheOutcome {
  /// The remote actor reference was served from cache.
  Hit,
  /// The remote actor reference was resolved by the provider.
  Miss,
}
