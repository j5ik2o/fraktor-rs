use alloc::string::String;

use crate::actor::actor_ref::ActorRef;

/// Outcome returned by a remote deployment hook.
#[derive(Debug)]
pub enum RemoteDeploymentOutcome {
  /// The target node created the actor and returned a remote actor ref.
  RemoteCreated(ActorRef),
  /// The target address is local, so actor-core should continue with local spawn.
  UseLocalDeployment,
  /// Remote deployment failed and actor-core must not fall back to local spawn.
  Failed(String),
}
