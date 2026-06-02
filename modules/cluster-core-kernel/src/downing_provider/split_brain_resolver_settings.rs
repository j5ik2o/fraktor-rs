//! Split Brain Resolver settings used for join compatibility checks.

use core::time::Duration;

use super::SplitBrainResolverStrategy;

/// Split Brain Resolver settings exposed by cluster configuration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SplitBrainResolverSettings {
  stable_after:           Duration,
  active_strategy:        SplitBrainResolverStrategy,
  down_all_when_unstable: Duration,
  static_quorum_size:     Option<usize>,
}

impl SplitBrainResolverSettings {
  /// Creates settings with explicit timing and active strategy.
  #[must_use]
  pub const fn new(
    stable_after: Duration,
    active_strategy: SplitBrainResolverStrategy,
    down_all_when_unstable: Duration,
  ) -> Self {
    Self { stable_after, active_strategy, down_all_when_unstable, static_quorum_size: None }
  }

  /// Returns settings with a fixed static quorum size.
  #[must_use]
  pub const fn with_static_quorum_size(mut self, static_quorum_size: usize) -> Self {
    self.static_quorum_size = Some(static_quorum_size);
    self
  }

  /// Returns how long membership must stay stable before decisions are applied.
  #[must_use]
  pub const fn stable_after(self) -> Duration {
    self.stable_after
  }

  /// Returns the active downing strategy identifier.
  #[must_use]
  pub const fn active_strategy(self) -> SplitBrainResolverStrategy {
    self.active_strategy
  }

  /// Returns the timeout after which every member is downed while unstable.
  #[must_use]
  pub const fn down_all_when_unstable(self) -> Duration {
    self.down_all_when_unstable
  }

  /// Returns the configured fixed quorum size for `StaticQuorum`.
  #[must_use]
  pub const fn static_quorum_size(self) -> Option<usize> {
    self.static_quorum_size
  }
}
