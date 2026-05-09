//! Effects emitted by [`crate::core::watcher::WatcherState`].

use alloc::vec::Vec;

use fraktor_actor_core_rs::actor::actor_path::ActorPath;

use crate::core::address::Address;

/// Side-effect instructions produced by [`crate::core::watcher::WatcherState::handle`].
///
/// Effects are applied by the adapter layer — the watcher state itself is a
/// pure transition function.
#[derive(Clone, Debug)]
pub enum WatcherEffect {
  /// Ask the adapter to send a heartbeat towards the given remote node.
  SendHeartbeat {
    /// Address of the peer that should receive the heartbeat.
    to: Address,
  },
  /// `target` was declared terminated; notify all watchers.
  NotifyTerminated {
    /// Actor that has been determined to be gone.
    target:   ActorPath,
    /// Watchers that must be notified of termination.
    watchers: Vec<ActorPath>,
  },
  /// The remote node has been quarantined and all its targets should be
  /// considered terminated.
  NotifyQuarantined {
    /// Quarantined remote node.
    node: Address,
  },
  /// Remote actor-system incarnation changed or was observed for the first
  /// time; re-issue watch messages for the node's targets.
  RewatchRemoteTargets {
    /// Remote node whose incarnation UID changed.
    node:    Address,
    /// Remote actor targets hosted by the node.
    targets: Vec<ActorPath>,
  },
}
