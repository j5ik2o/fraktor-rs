//! Drift observation states.

use core::time::Duration;

/// Drift observation outcome.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DriftStatus {
  /// Drift stayed within the configured budget.
  WithinBudget,
  /// Drift exceeded the allowed percentage.
  Exceeded {
    /// Measured drift between the deadline and the actual instant.
    observed: Duration,
  },
}
