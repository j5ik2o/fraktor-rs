//! Tracks restart attempts for supervised actors.

extern crate alloc;

use alloc::vec::Vec;
use core::time::Duration;

/// Maintains failure timestamps to enforce restart limits.
pub struct RestartStatistics {
  failures: Vec<Duration>,
}

impl RestartStatistics {
  /// Creates an empty statistics container.
  #[must_use]
  pub fn new() -> Self {
    Self { failures: Vec::new() }
  }

  /// Records a failure occurring at `now`, returning the total failures within the provided window.
  ///
  /// When `window` is zero, all failures are retained and counted.
  pub fn record_failure(&mut self, now: Duration, window: Duration, max_history: Option<u32>) -> usize {
    self.prune(window, now);
    self.failures.push(now);

    let count = self.failures.len();
    if let Some(limit) = max_history {
      let limit = limit as usize;
      if limit > 0 && self.failures.len() > limit {
        let excess = self.failures.len() - limit;
        self.failures.drain(0..excess);
      }
    }
    count
  }

  /// Returns the number of recorded failures.
  #[must_use]
  pub fn failure_count(&self) -> usize {
    self.failures.len()
  }

  /// Returns the number of failures that occurred within `window` from `now`.
  #[must_use]
  pub fn failures_within(&self, window: Duration, now: Duration) -> usize {
    if window.is_zero() {
      return self.failures.len();
    }
    let threshold = now.saturating_sub(window);
    self.failures.iter().filter(|&&timestamp| timestamp >= threshold).count()
  }

  /// Clears all tracked failures.
  pub fn reset(&mut self) {
    self.failures.clear();
  }

  fn prune(&mut self, window: Duration, now: Duration) {
    if window.is_zero() {
      return;
    }
    let threshold = now.saturating_sub(window);
    self.failures.retain(|&timestamp| timestamp >= threshold);
  }
}

impl Default for RestartStatistics {
  fn default() -> Self {
    Self::new()
  }
}

#[cfg(test)]
mod tests {
  use core::time::Duration;

  use super::RestartStatistics;

  #[test]
  fn record_prunes_outdated_failures() {
    let mut stats = RestartStatistics::new();
    let window = Duration::from_secs(5);

    assert_eq!(stats.record_failure(Duration::from_secs(1), window, None), 1);
    assert_eq!(stats.record_failure(Duration::from_secs(3), window, None), 2);
    assert_eq!(stats.record_failure(Duration::from_secs(9), window, None), 1);
    assert_eq!(stats.failures_within(window, Duration::from_secs(9)), 1);
  }

  #[test]
  fn record_limits_history_capacity() {
    let mut stats = RestartStatistics::new();
    let window = Duration::ZERO;

    assert_eq!(stats.record_failure(Duration::from_secs(1), window, Some(2)), 1);
    assert_eq!(stats.record_failure(Duration::from_secs(2), window, Some(2)), 2);
    assert_eq!(stats.record_failure(Duration::from_secs(3), window, Some(2)), 3);
    assert_eq!(stats.failure_count(), 2);
  }

  #[test]
  fn reset_clears_failures() {
    let mut stats = RestartStatistics::new();
    stats.record_failure(Duration::from_secs(1), Duration::ZERO, None);
    stats.reset();
    assert_eq!(stats.failure_count(), 0);
  }
}
