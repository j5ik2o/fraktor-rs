use core::time::Duration;

use super::{RestartCounter, duration_millis};

#[test]
fn restart_returns_true_within_budget() {
  let mut counter = RestartCounter::new(2, Duration::from_millis(100));

  assert!(counter.restart(0));
  assert!(counter.restart(50));
  assert_eq!(counter.count(), 2);
}

#[test]
fn restart_returns_false_when_budget_exhausted() {
  let mut counter = RestartCounter::new(2, Duration::from_millis(100));

  assert!(counter.restart(0));
  assert!(counter.restart(10));
  assert!(!counter.restart(20));
}

#[test]
fn restart_resets_count_after_window_elapses() {
  let mut counter = RestartCounter::new(1, Duration::from_millis(50));

  assert!(counter.restart(0));
  assert!(!counter.restart(10));
  // 50ms 経過後は最初の試行扱いに戻り、再び 1 回までは許容される。
  assert!(counter.restart(60));
}

#[test]
fn duration_millis_floors_sub_millisecond_to_one_millisecond() {
  // `as_millis()` は 1ms 未満を 0 に切り捨てるが、それでは deadline_ms == now_ms に固定され
  // restart budget が無効化されるため、非ゼロ Duration は最低 1ms に引き上げる。
  assert_eq!(duration_millis(Duration::from_nanos(1)), 1);
  assert_eq!(duration_millis(Duration::from_micros(500)), 1);
}

#[test]
fn duration_millis_keeps_zero_for_zero_duration() {
  assert_eq!(duration_millis(Duration::ZERO), 0);
}

#[test]
fn reset_returns_budget_to_full_for_next_failure_cycle() {
  let mut counter = RestartCounter::new(2, Duration::from_millis(100));

  assert!(counter.restart(0));
  assert!(counter.restart(10));
  assert!(!counter.restart(20), "third restart inside window should exceed budget");

  counter.reset();

  // reset 後は同じ window 内でも満額の budget が戻る。
  assert!(counter.restart(30));
  assert!(counter.restart(40));
  assert!(!counter.restart(50));
}

#[test]
fn restart_with_sub_millisecond_timeout_still_enforces_budget() {
  let mut counter = RestartCounter::new(1, Duration::from_nanos(1));

  assert!(counter.restart(0));
  assert!(!counter.restart(0));
}
