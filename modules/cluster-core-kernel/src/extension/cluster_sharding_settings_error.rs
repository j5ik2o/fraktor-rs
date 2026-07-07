//! Cluster sharding settings validation errors.

use core::{
  error::Error,
  fmt::{self, Formatter, Result as FmtResult},
};

#[cfg(test)]
#[path = "cluster_sharding_settings_error_test.rs"]
mod tests;

/// Cluster sharding settings validation errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClusterShardingSettingsError {
  /// Number of shards is zero.
  ZeroNumberOfShards,
  /// Hand-over retry interval is zero.
  ZeroHandOverRetryInterval,
  /// Retry interval is zero.
  ZeroRetryInterval,
  /// Rebalance interval is zero.
  ZeroRebalanceInterval,
  /// Shard region query timeout is zero.
  ZeroShardRegionQueryTimeout,
  /// Hand-off timeout is zero.
  ZeroHandOffTimeout,
  /// Entity restart backoff is zero.
  ZeroEntityRestartBackoff,
  /// Passivation active entity limit is zero.
  ZeroActiveEntityLimit,
  /// Passivation idle timeout is zero.
  ZeroIdleTimeout,
  /// Remember entities cannot be combined with a non-disabled passivation strategy.
  PassivationWithRememberEntities,
}

impl fmt::Display for ClusterShardingSettingsError {
  fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
    match self {
      | Self::ZeroNumberOfShards => f.write_str("number of shards must be greater than zero"),
      | Self::ZeroHandOverRetryInterval => f.write_str("hand-over retry interval must be greater than zero"),
      | Self::ZeroRetryInterval => f.write_str("retry interval must be greater than zero"),
      | Self::ZeroRebalanceInterval => f.write_str("rebalance interval must be greater than zero"),
      | Self::ZeroShardRegionQueryTimeout => f.write_str("shard region query timeout must be greater than zero"),
      | Self::ZeroHandOffTimeout => f.write_str("hand-off timeout must be greater than zero"),
      | Self::ZeroEntityRestartBackoff => f.write_str("entity restart backoff must be greater than zero"),
      | Self::ZeroActiveEntityLimit => f.write_str("passivation active entity limit must be greater than zero"),
      | Self::ZeroIdleTimeout => f.write_str("passivation idle timeout must be greater than zero"),
      | Self::PassivationWithRememberEntities => {
        f.write_str("remember entities cannot be enabled together with passivation")
      },
    }
  }
}

impl Error for ClusterShardingSettingsError {}
