//! Tick metrics collection mode.

use core::time::Duration;

/// Mode for tick metrics collection and publishing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TickMetricsMode {
  /// Automatically publish metrics at regular intervals.
  AutoPublish {
    /// Interval between metric publications.
    interval: Duration,
  },
  /// Only publish metrics on demand via snapshot().
  OnDemand,
}

impl Default for TickMetricsMode {
  fn default() -> Self {
    Self::AutoPublish { interval: Duration::from_secs(1) }
  }
}
