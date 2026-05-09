//! Initial cluster state replay mode for event subscriptions.

/// Controls how `ClusterApi::subscribe` emits initial-state data to new subscribers.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClusterSubscriptionInitialStateMode {
  /// Replays all buffered matching events (historical event stream semantics).
  AsEvents,
  /// Sends one `CurrentClusterState` snapshot as the initial message.
  AsSnapshot,
}
