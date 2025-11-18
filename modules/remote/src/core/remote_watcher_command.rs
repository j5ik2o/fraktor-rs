//! Commands handled by the remote watcher daemon.

use fraktor_actor_rs::core::actor_prim::{Pid, actor_path::ActorPathParts};

/// Commands handled by the remote watcher daemon.
#[allow(dead_code)]
#[derive(Clone)]
pub enum RemoteWatcherCommand {
  /// Requests association/watch for the provided remote path.
  Watch {
    /// Path metadata of the remote actor to monitor.
    target:  ActorPathParts,
    /// Local watcher PID issuing the request.
    watcher: Pid,
  },
  /// Drops an existing remote watch request.
  Unwatch {
    /// Path metadata of the remote actor to stop monitoring.
    target:  ActorPathParts,
    /// Local watcher PID cancelling the request.
    watcher: Pid,
  },
}
