extern crate std;

use core::time::Duration;
use std::thread;

use super::CircuitBreaker;
use crate::std::pattern::circuit_breaker_state::CircuitBreakerState;

#[test]
#[should_panic(expected = "max_failures must be greater than zero")]
fn rejects_zero_max_failures() {
  // max_failures == 0 は意味が曖昧なため拒否される
  let _ = CircuitBreaker::new(0, Duration::from_millis(100));
}

#[test]
fn new_starts_in_closed_state() {
  let cb = CircuitBreaker::new(3, Duration::from_millis(100));
  assert_eq!(cb.state(), CircuitBreakerState::Closed);
  assert_eq!(cb.failure_count(), 0);
  assert_eq!(cb.max_failures(), 3);
  assert_eq!(cb.reset_timeout(), Duration::from_millis(100));
}

#[test]
fn stays_closed_on_success() {
  let mut cb = CircuitBreaker::new(3, Duration::from_millis(100));
  cb.record_success();
  assert_eq!(cb.state(), CircuitBreakerState::Closed);
  assert_eq!(cb.failure_count(), 0);
}

#[test]
fn stays_closed_when_failures_below_threshold() {
  let mut cb = CircuitBreaker::new(3, Duration::from_millis(100));
  cb.record_failure();
  assert_eq!(cb.failure_count(), 1);
  assert_eq!(cb.state(), CircuitBreakerState::Closed);

  cb.record_failure();
  assert_eq!(cb.failure_count(), 2);
  assert_eq!(cb.state(), CircuitBreakerState::Closed);
}

#[test]
fn transitions_to_open_at_max_failures() {
  let mut cb = CircuitBreaker::new(3, Duration::from_millis(100));
  cb.record_failure();
  cb.record_failure();
  cb.record_failure();
  assert_eq!(cb.state(), CircuitBreakerState::Open);
}

#[test]
fn resets_failure_count_on_success_in_closed() {
  let mut cb = CircuitBreaker::new(3, Duration::from_millis(100));
  cb.record_failure();
  cb.record_failure();
  assert_eq!(cb.failure_count(), 2);

  cb.record_success();
  assert_eq!(cb.failure_count(), 0);
  assert_eq!(cb.state(), CircuitBreakerState::Closed);
}

#[test]
fn open_rejects_calls() {
  let mut cb = CircuitBreaker::new(1, Duration::from_secs(10));
  cb.record_failure();
  assert_eq!(cb.state(), CircuitBreakerState::Open);

  let err = match cb.is_call_permitted() {
    | Err(e) => e,
    | Ok(()) => panic!("expected Err but got Ok(())"),
  };
  assert!(err.remaining() > Duration::ZERO);
}

#[test]
fn open_transitions_to_half_open_after_reset_timeout() {
  let mut cb = CircuitBreaker::new(1, Duration::from_millis(10));
  cb.record_failure();
  assert_eq!(cb.state(), CircuitBreakerState::Open);

  thread::sleep(Duration::from_millis(20));

  assert!(cb.is_call_permitted().is_ok());
  assert_eq!(cb.state(), CircuitBreakerState::HalfOpen);
}

#[test]
fn half_open_rejects_second_call() {
  let mut cb = CircuitBreaker::new(1, Duration::from_millis(10));
  cb.record_failure();

  thread::sleep(Duration::from_millis(20));

  // 最初の呼び出しは許可される（HalfOpen に遷移）
  assert!(cb.is_call_permitted().is_ok());
  assert_eq!(cb.state(), CircuitBreakerState::HalfOpen);

  // 2回目の呼び出しは拒否される
  assert!(cb.is_call_permitted().is_err());
}

#[test]
fn half_open_success_transitions_to_closed() {
  let mut cb = CircuitBreaker::new(1, Duration::from_millis(10));
  cb.record_failure();

  thread::sleep(Duration::from_millis(20));

  assert!(cb.is_call_permitted().is_ok());
  cb.record_success();
  assert_eq!(cb.state(), CircuitBreakerState::Closed);
  assert_eq!(cb.failure_count(), 0);
}

#[test]
fn half_open_failure_transitions_to_open() {
  let mut cb = CircuitBreaker::new(1, Duration::from_millis(10));
  cb.record_failure();

  thread::sleep(Duration::from_millis(20));

  assert!(cb.is_call_permitted().is_ok());
  cb.record_failure();
  assert_eq!(cb.state(), CircuitBreakerState::Open);
}

#[test]
fn closed_always_permits() {
  let mut cb = CircuitBreaker::new(5, Duration::from_secs(1));
  for _ in 0..10 {
    assert!(cb.is_call_permitted().is_ok());
    cb.record_success();
  }
  assert_eq!(cb.state(), CircuitBreakerState::Closed);
}

#[test]
fn max_failures_one_trips_on_first_failure() {
  let mut cb = CircuitBreaker::new(1, Duration::from_millis(100));
  cb.record_failure();
  assert_eq!(cb.state(), CircuitBreakerState::Open);
}

#[test]
fn recovery_cycle_closed_open_half_open_closed() {
  let mut cb = CircuitBreaker::new(2, Duration::from_millis(10));

  // Closed → Open
  cb.record_failure();
  cb.record_failure();
  assert_eq!(cb.state(), CircuitBreakerState::Open);

  // Open → HalfOpen
  thread::sleep(Duration::from_millis(20));
  assert!(cb.is_call_permitted().is_ok());
  assert_eq!(cb.state(), CircuitBreakerState::HalfOpen);

  // HalfOpen → Closed
  cb.record_success();
  assert_eq!(cb.state(), CircuitBreakerState::Closed);
  assert_eq!(cb.failure_count(), 0);

  // 再度失敗・回復が可能であることを確認
  cb.record_failure();
  cb.record_failure();
  assert_eq!(cb.state(), CircuitBreakerState::Open);
}
