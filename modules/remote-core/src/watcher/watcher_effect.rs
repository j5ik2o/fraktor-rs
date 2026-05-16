//! Effects emitted by [`crate::watcher::WatcherState`].

use alloc::{string::String, vec::Vec};

use fraktor_actor_core_kernel_rs::actor::actor_path::ActorPath;

use crate::address::Address;

/// Side-effect instructions produced by [`crate::watcher::WatcherState::handle`].
///
/// Effects are applied by the adapter layer — the watcher state itself is a
/// pure transition function.
#[derive(Clone, Debug)]
pub enum WatcherEffect {
  /// Ask the adapter to send a remote `Watch` system message.
  SendWatch {
    /// Remote actor being watched.
    target:  ActorPath,
    /// Actor that receives the termination notification.
    watcher: ActorPath,
  },
  /// Ask the adapter to send a remote `Unwatch` system message.
  SendUnwatch {
    /// Remote actor that was watched.
    target:  ActorPath,
    /// Actor that should stop receiving termination notifications.
    watcher: ActorPath,
  },
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
  /// The remote node was declared terminated for event-stream publication.
  AddressTerminated {
    /// Terminated remote node.
    node:               Address,
    /// Human-readable termination reason metadata.
    reason:             String,
    /// Monotonic millis timestamp when the termination was observed.
    observed_at_millis: u64,
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
    /// Remote actor target / watcher pairs that must be watched again.
    watches: Vec<(ActorPath, ActorPath)>,
  },
}
