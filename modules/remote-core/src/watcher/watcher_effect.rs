//! Effects emitted by [`crate::watcher::WatcherState`].

use alloc::vec::Vec;

use fraktor_actor_core_rs::core::kernel::actor::actor_path::ActorPath;

use crate::address::Address;

/// Side-effect instructions produced by [`crate::watcher::WatcherState::handle`].
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
}
