use core::time::Duration;

use super::RestartStatistics;

#[test]
fn record_failure_counts_within_window() {
  let mut stats = RestartStatistics::new();
  stats.record_failure(Duration::from_secs(1), Duration::from_secs(5), Some(3));
  stats.record_failure(Duration::from_secs(3), Duration::from_secs(5), Some(3));
  assert_eq!(stats.failure_count(), 2);
  assert_eq!(stats.failures_within(Duration::from_secs(2), Duration::from_secs(4)), 1);
}

#[test]
fn exceeding_history_prunes_old_entries() {
  let mut stats = RestartStatistics::new();
  stats.record_failure(Duration::from_secs(1), Duration::from_secs(5), Some(1));
  stats.record_failure(Duration::from_secs(2), Duration::from_secs(5), Some(1));
  assert_eq!(stats.failure_count(), 1);
}

#[test]
fn reset_clears_failures() {
  let mut stats = RestartStatistics::new();
  stats.record_failure(Duration::from_secs(1), Duration::from_secs(5), None);
  stats.reset();
  assert_eq!(stats.failure_count(), 0);
}
