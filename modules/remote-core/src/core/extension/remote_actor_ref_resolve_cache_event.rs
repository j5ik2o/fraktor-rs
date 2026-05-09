//! Remote actor ref resolve cache event payload.

#[cfg(test)]
mod tests;

use fraktor_actor_core_kernel_rs::actor::actor_path::ActorPath;

use crate::core::extension::RemoteActorRefResolveCacheOutcome;

/// Extension event name used for remote actor ref resolve cache observations.
pub const REMOTE_ACTOR_REF_RESOLVE_CACHE_EXTENSION: &str = "remote.actor-ref-resolve-cache";

/// Event payload emitted when a remote actor ref resolve cache is observed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RemoteActorRefResolveCacheEvent {
  path:    ActorPath,
  outcome: RemoteActorRefResolveCacheOutcome,
}

impl RemoteActorRefResolveCacheEvent {
  /// Creates a new cache observation event.
  #[must_use]
  pub const fn new(path: ActorPath, outcome: RemoteActorRefResolveCacheOutcome) -> Self {
    Self { path, outcome }
  }

  /// Returns the actor path that was resolved.
  #[must_use]
  pub const fn path(&self) -> &ActorPath {
    &self.path
  }

  /// Returns the observed cache outcome.
  #[must_use]
  pub const fn outcome(&self) -> RemoteActorRefResolveCacheOutcome {
    self.outcome
  }
}
