//! Commands handled by the remote watcher daemon.

#[cfg(feature = "tokio-transport")]
use alloc::string::String;

use fraktor_actor_rs::core::actor::{Pid, actor_path::ActorPathParts};

use super::{heartbeat::Heartbeat, heartbeat_rsp::HeartbeatRsp};

/// Commands handled by the remote watcher daemon.
#[allow(dead_code)]
#[derive(Clone)]
pub(crate) enum RemoteWatcherCommand {
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
  /// Heartbeat probe received from a remote authority.
  Heartbeat {
    /// Heartbeat payload.
    heartbeat: Heartbeat,
  },
  /// Heartbeat response received from a remote authority.
  HeartbeatRsp {
    /// Heartbeat response payload.
    heartbeat_rsp: HeartbeatRsp,
  },
  /// Triggers failure-detector polling for watched authorities.
  ReapUnreachable,
  /// Triggers a heartbeat tick for watched authorities.
  HeartbeatTick,
}

impl RemoteWatcherCommand {
  /// Creates a heartbeat probe command for the authority.
  #[must_use]
  #[cfg(feature = "tokio-transport")]
  pub(crate) fn heartbeat(authority: impl Into<String>) -> Self {
    Self::Heartbeat { heartbeat: Heartbeat::new(authority) }
  }

  /// Creates a heartbeat response command for the authority.
  #[must_use]
  #[cfg(feature = "tokio-transport")]
  pub(crate) fn heartbeat_rsp(authority: impl Into<String>, uid: u64) -> Self {
    Self::HeartbeatRsp { heartbeat_rsp: HeartbeatRsp::new(authority, uid) }
  }

  /// Creates a command that runs failure-detector polling.
  #[must_use]
  #[cfg(feature = "tokio-transport")]
  pub(crate) const fn reap_unreachable() -> Self {
    Self::ReapUnreachable
  }

  /// Creates a command that triggers periodic heartbeat processing.
  #[must_use]
  #[cfg(feature = "tokio-transport")]
  pub(crate) const fn heartbeat_tick() -> Self {
    Self::HeartbeatTick
  }
}
