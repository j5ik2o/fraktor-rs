//! Rebalance strategy selection and settings.

use core::time::Duration;

#[cfg(test)]
#[path = "rebalance_strategy_test.rs"]
mod tests;

use super::rebalance_strategy_settings::RebalanceStrategySettings;

/// Rebalance strategy used by shard coordinators.
#[derive(Debug, Clone, PartialEq)]
pub enum RebalanceStrategy {
  /// Rebalance using least-shard allocation.
  LeastShards(RebalanceStrategySettings),
  /// Rebalancing is disabled.
  Disabled,
}

impl RebalanceStrategy {
  /// Creates the default least-shard strategy.
  #[must_use]
  pub const fn least_shards_default() -> Self {
    Self::LeastShards(RebalanceStrategySettings::new())
  }

  /// Returns whether rebalancing is enabled.
  #[must_use]
  pub const fn is_enabled(&self) -> bool {
    !matches!(self, Self::Disabled)
  }

  /// Returns rebalance settings when the strategy uses least-shard allocation.
  #[must_use]
  pub const fn least_shards_settings(&self) -> Option<&RebalanceStrategySettings> {
    match self {
      | Self::LeastShards(settings) => Some(settings),
      | Self::Disabled => None,
    }
  }

  /// Returns the configured rebalance interval when rebalancing is enabled.
  #[must_use]
  pub const fn rebalance_interval(&self, configured: Duration) -> Duration {
    match self {
      | Self::Disabled => Duration::ZERO,
      | Self::LeastShards(_) => configured,
    }
  }
}

impl Default for RebalanceStrategy {
  fn default() -> Self {
    Self::least_shards_default()
  }
}
