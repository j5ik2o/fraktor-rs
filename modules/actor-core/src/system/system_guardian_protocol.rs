//! Public protocol for interacting with the system guardian.

use crate::{RuntimeToolbox, actor_prim::actor_ref::ActorRefGeneric};

/// Messages understood by the system guardian actor.
#[derive(Clone)]
pub enum SystemGuardianProtocol<TB: RuntimeToolbox + 'static> {
  /// Registers the provided actor as a termination hook participant.
  RegisterTerminationHook(ActorRefGeneric<TB>),
  /// Sent to hook actors to begin graceful shutdown.
  TerminationHook,
  /// Indicates that the provided hook actor has completed cleanup.
  TerminationHookDone(ActorRefGeneric<TB>),
  /// Forces all pending hooks to complete immediately (best-effort).
  ForceTerminateHooks,
}
