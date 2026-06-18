//! Cluster sharding state-store compatibility mode.

#[cfg(test)]
#[path = "cluster_sharding_state_store_mode_test.rs"]
mod tests;

/// Advertised state-store mode used for sharding join compatibility.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ClusterShardingStateStoreMode {
  /// Uses distributed data for shard state, matching Pekko's `ddata` mode.
  #[default]
  DData,
  /// Uses durable persistence for shard state, matching Pekko's `persistence` mode.
  Persistence,
}

impl ClusterShardingStateStoreMode {
  /// Returns the stable configuration value for this state-store mode.
  #[must_use]
  pub const fn as_str(self) -> &'static str {
    match self {
      | Self::DData => "ddata",
      | Self::Persistence => "persistence",
    }
  }
}
