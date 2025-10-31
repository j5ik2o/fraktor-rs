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
