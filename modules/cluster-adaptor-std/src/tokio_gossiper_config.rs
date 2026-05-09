//! Configuration for Tokio gossiper.

use core::time::Duration;

/// Configuration for Tokio gossiper.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokioGossiperConfig {
  /// Interval between gossip ticks.
  pub tick_interval:   Duration,
  /// Timer resolution for `TimerInstant`.
  pub tick_resolution: Duration,
}

impl TokioGossiperConfig {
  /// Creates a new configuration.
  #[must_use]
  pub const fn new(tick_interval: Duration, tick_resolution: Duration) -> Self {
    Self { tick_interval, tick_resolution }
  }
}
