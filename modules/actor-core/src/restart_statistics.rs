//! Tracks restart attempts for supervised actors.

use core::time::Duration;

#[derive(Debug, Clone, Copy)]
/// Keeps track of restart attempts and budgets for a single actor.
pub struct RestartStatistics {
  max_restarts: u32,
  window:       Duration,
  failures:     u32,
}

impl RestartStatistics {
  #[must_use]
  /// Creates a new statistics tracker with a restart budget and time window.
  pub const fn new(max_restarts: u32, window: Duration) -> Self {
    Self { max_restarts, window, failures: 0 }
  }

  #[must_use]
  /// Registers a failure and returns `true` when a restart is still permitted.
  #[allow(clippy::missing_const_for_fn)]
  pub fn allow_restart(&mut self) -> bool {
    if self.max_restarts == 0 {
      return false;
    }
    if self.failures < self.max_restarts {
      self.failures += 1;
      true
    } else {
      false
    }
  }

  /// Resets the failure counter, typically after the window has elapsed.
  #[allow(clippy::missing_const_for_fn)]
  pub fn reset(&mut self) {
    self.failures = 0;
  }

  #[must_use]
  /// Returns the remaining restart attempts.
  pub const fn remaining(&self) -> u32 {
    self.max_restarts.saturating_sub(self.failures)
  }

  #[must_use]
  /// Returns the configured restart budget.
  pub const fn max_restarts(&self) -> u32 {
    self.max_restarts
  }

  #[must_use]
  /// Returns the time window in which the restart budget applies.
  pub const fn window(&self) -> Duration {
    self.window
  }
}
