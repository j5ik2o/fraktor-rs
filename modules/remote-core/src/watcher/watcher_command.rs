//! Commands driving [`crate::watcher::WatcherState`].

use fraktor_actor_core_kernel_rs::actor::actor_path::ActorPath;

use crate::address::Address;

/// Input events consumed by [`crate::watcher::WatcherState::handle`].
///
/// All timestamps are **monotonic millis**. Callers must pass values from the
/// same monotonic source used by the inner
/// [`crate::failure_detector::PhiAccrualFailureDetector`] (typically
/// `std::time::Instant` differences in the adapter layer). Wall-clock values
/// are not supported.
#[derive(Clone, Debug)]
pub enum WatcherCommand {
  /// Start watching `target` on behalf of `watcher`.
  Watch {
    /// Actor being watched (must be a remote path).
    target:  ActorPath,
    /// Actor that wants to be notified on termination.
    watcher: ActorPath,
  },
  /// Stop watching `target` on behalf of `watcher`.
  Unwatch {
    /// Previously watched actor.
    target:  ActorPath,
    /// Actor that was receiving notifications.
    watcher: ActorPath,
  },
  /// A heartbeat frame arrived from `from` at monotonic time `now` (millis).
  HeartbeatReceived {
    /// Address of the remote node that emitted the heartbeat.
    from: Address,
    /// Monotonic millis at which the heartbeat was observed.
    now:  u64,
  },
  /// A heartbeat response arrived from `from` carrying the remote actor-system
  /// incarnation UID.
  HeartbeatResponseReceived {
    /// Address of the remote node that emitted the heartbeat response.
    from: Address,
    /// Actor-system incarnation UID reported by the remote node.
    uid:  u64,
    /// Monotonic millis at which the heartbeat response was observed.
    now:  u64,
  },
  /// Periodic tick driving failure-detector evaluation at monotonic time
  /// `now` (millis).
  HeartbeatTick {
    /// Monotonic millis at which the tick fires.
    now: u64,
  },
}
