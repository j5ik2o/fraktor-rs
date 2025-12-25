//! Errors raised while resolving cluster identities.

use alloc::string::String;

use fraktor_actor_rs::core::system::ActorRefResolveError;

/// Errors returned by cluster identity resolution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClusterResolveError {
  /// Cluster has not been started.
  ClusterNotStarted,
  /// Requested kind is not registered.
  KindNotRegistered {
    /// Requested kind name.
    kind: String,
  },
  /// Identity lookup did not return a PID.
  LookupFailed,
  /// Identity lookup is still pending.
  LookupPending,
  /// PID string could not be parsed into an actor path.
  InvalidPidFormat {
    /// Raw PID string.
    pid:    String,
    /// Failure reason.
    reason: String,
  },
  /// Actor system failed to resolve the actor reference.
  ActorRefResolve(ActorRefResolveError),
}
