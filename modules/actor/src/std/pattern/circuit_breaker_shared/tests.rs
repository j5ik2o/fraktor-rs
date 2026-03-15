extern crate std;

use core::time::Duration;
use std::thread;

use super::CircuitBreakerShared;
use crate::std::pattern::{
  circuit_breaker_call_error::CircuitBreakerCallError, circuit_breaker_state::CircuitBreakerState,
};

#[test]
fn new_starts_closed() {
  let cb = CircuitBreakerShared::new(3, Duration::from_millis(100));
  assert_eq!(cb.state(), CircuitBreakerState::Closed);
  assert_eq!(cb.failure_count(), 0);
}

#[test]
fn clone_shares_state() {
  let cb1 = CircuitBreakerShared::new(3, Duration::from_millis(100));
  let cb2 = cb1.clone();

  // Mutating through cb1 is visible through cb2.
  fraktor_utils_rs::core::sync::SharedAccess::with_write(&cb1, |inner| {
    inner.record_failure();
  });
  assert_eq!(cb2.failure_count(), 1);
}

#[tokio::test]
async fn call_succeeds_in_closed() {
  let cb = CircuitBreakerShared::new(3, Duration::from_millis(100));

  let result = cb.call(|| async { Ok::<_, &str>(42) }).await;
  match result {
    | Ok(value) => assert_eq!(value, 42),
    | Err(err) => panic!("期待値: Ok(42), 実際: Err({err:?})"),
  }
  assert_eq!(cb.state(), CircuitBreakerState::Closed);
}

#[tokio::test]
async fn call_records_failure() {
  let cb = CircuitBreakerShared::new(2, Duration::from_millis(100));

  let result = cb.call(|| async { Err::<(), _>("oops") }).await;
  assert!(matches!(result, Err(CircuitBreakerCallError::Failed("oops"))));
  assert_eq!(cb.failure_count(), 1);
}

#[tokio::test]
async fn call_trips_after_max_failures() {
  let cb = CircuitBreakerShared::new(2, Duration::from_millis(100));

  let _ = cb.call(|| async { Err::<(), _>("a") }).await;
  let _ = cb.call(|| async { Err::<(), _>("b") }).await;
  assert_eq!(cb.state(), CircuitBreakerState::Open);

  let result = cb.call(|| async { Ok::<_, &str>(1) }).await;
  assert!(matches!(result, Err(CircuitBreakerCallError::Open(_))));
}

#[tokio::test]
async fn call_recovers_after_reset_timeout() {
  let cb = CircuitBreakerShared::new(1, Duration::from_millis(10));

  let _ = cb.call(|| async { Err::<(), _>("fail") }).await;
  assert_eq!(cb.state(), CircuitBreakerState::Open);

  thread::sleep(Duration::from_millis(20));

  let result = cb.call(|| async { Ok::<_, &str>(99) }).await;
  match result {
    | Ok(value) => assert_eq!(value, 99),
    | Err(err) => panic!("期待値: Ok(99), 実際: Err({err:?})"),
  }
  assert_eq!(cb.state(), CircuitBreakerState::Closed);
}

#[tokio::test]
async fn half_open_failure_reopens() {
  let cb = CircuitBreakerShared::new(1, Duration::from_millis(10));

  let _ = cb.call(|| async { Err::<(), _>("fail") }).await;

  thread::sleep(Duration::from_millis(20));

  let result = cb.call(|| async { Err::<(), _>("still broken") }).await;
  assert!(matches!(result, Err(CircuitBreakerCallError::Failed("still broken"))));
  assert_eq!(cb.state(), CircuitBreakerState::Open);
}

#[tokio::test]
async fn open_error_contains_remaining_duration() {
  let cb = CircuitBreakerShared::new(1, Duration::from_secs(10));

  let _ = cb.call(|| async { Err::<(), _>("fail") }).await;

  let result = cb.call(|| async { Ok::<_, &str>(1) }).await;
  match result {
    | Err(CircuitBreakerCallError::Open(err)) => {
      assert!(err.remaining() > Duration::ZERO);
      assert!(err.remaining() <= Duration::from_secs(10));
    },
    | other => panic!("expected Open error, got {:?}", other),
  }
}
