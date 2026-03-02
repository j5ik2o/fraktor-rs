//! Public protocol for interacting with the system guardian.

use crate::core::actor::actor_ref::ActorRef;

/// Messages understood by the system guardian actor.
#[derive(Clone)]
pub enum SystemGuardianProtocol {
  /// Registers the provided actor as a termination hook participant.
  RegisterTerminationHook(ActorRef),
  /// Sent to hook actors to begin graceful shutdown.
  TerminationHook,
  /// Indicates that the provided hook actor has completed cleanup.
  TerminationHookDone(ActorRef),
  /// Forces all pending hooks to complete immediately (best-effort).
  ForceTerminateHooks,
}
