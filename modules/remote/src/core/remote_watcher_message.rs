//! Message protocol for remote watcher daemon.

use fraktor_actor_rs::core::actor_prim::actor_path::ActorPathParts;

/// Command handled by the remote watcher daemon.
#[derive(Clone)]
pub enum RemoteWatcherMessage {
  /// Registers a watch request for the specified remote path.
  Watch {
    /// Actor path to watch.
    target: ActorPathParts,
  },
  /// Cancels a previously registered watch.
  Unwatch {
    /// Actor path to unwatch.
    target: ActorPathParts,
  },
  /// Requests the latest endpoint snapshot.
  Snapshot,
}
