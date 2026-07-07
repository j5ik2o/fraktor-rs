//! Limits applied during one rebalance round.

/// Limits applied during one rebalance round.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RebalanceStrategySettings {
  /// Maximum number of shards rebalanced in one round.
  ///
  /// When zero, only the relative limit applies and at least one shard is rebalanced per round.
  absolute_limit: u32,
  /// Fraction of known shards that may be rebalanced in one round.
  relative_limit: f64,
}

impl RebalanceStrategySettings {
  /// Creates settings with Pekko-compatible defaults.
  #[must_use]
  pub const fn new() -> Self {
    Self { absolute_limit: 0, relative_limit: 0.1 }
  }

  /// Creates settings with explicit limits.
  #[must_use]
  pub const fn with_limits(absolute_limit: u32, relative_limit: f64) -> Self {
    Self { absolute_limit, relative_limit }
  }

  /// Sets the absolute rebalance limit.
  #[must_use]
  pub const fn with_absolute_limit(mut self, absolute_limit: u32) -> Self {
    self.absolute_limit = absolute_limit;
    self
  }

  /// Sets the relative rebalance limit.
  #[must_use]
  pub const fn with_relative_limit(mut self, relative_limit: f64) -> Self {
    self.relative_limit = relative_limit;
    self
  }

  /// Returns the absolute rebalance limit.
  #[must_use]
  pub const fn absolute_limit(&self) -> u32 {
    self.absolute_limit
  }

  /// Returns the relative rebalance limit.
  #[must_use]
  pub const fn relative_limit(&self) -> f64 {
    self.relative_limit
  }

  /// Returns the effective shard limit for one rebalance round.
  #[must_use]
  pub fn rebalance_limit(&self, number_of_shards: usize) -> usize {
    let relative = (self.relative_limit * number_of_shards as f64) as u32;
    let capped = if self.absolute_limit == 0 { relative } else { relative.min(self.absolute_limit) };
    core::cmp::max(1, capped as usize)
  }
}

impl Default for RebalanceStrategySettings {
  fn default() -> Self {
    Self::new()
  }
}
