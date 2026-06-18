//! Cluster sharding state-store compatibility mode.

#[cfg(test)]
#[path = "cluster_sharding_state_store_mode_test.rs"]
mod tests;

/// Advertised state-store mode used for sharding join compatibility.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ClusterShardingStateStoreMode {
  /// Keeps shard and placement state in the current runtime process.
  #[default]
  InMemory,
  /// Requires durable shard and placement state supplied by the embedding runtime.
  Durable,
}

impl ClusterShardingStateStoreMode {
  /// Returns the stable configuration value for this state-store mode.
  #[must_use]
  pub const fn as_str(self) -> &'static str {
    match self {
      | Self::InMemory => "in-memory",
      | Self::Durable => "durable",
    }
  }
}
