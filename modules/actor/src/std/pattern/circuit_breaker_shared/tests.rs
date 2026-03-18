extern crate std;

use core::{
  sync::atomic::{AtomicU64, Ordering},
  time::Duration,
};
use std::{sync::Arc, time::Instant};

use fraktor_utils_rs::core::sync::{ArcShared, RuntimeMutex};

use super::CircuitBreakerShared;
use crate::std::pattern::{
  circuit_breaker::CircuitBreaker, circuit_breaker_call_error::CircuitBreakerCallError,
  circuit_breaker_state::CircuitBreakerState,
};

#[derive(Clone)]
struct FakeClock {
  base:          Instant,
  offset_millis: Arc<AtomicU64>,
}

impl FakeClock {
  fn new() -> Self {
    Self { base: Instant::now(), offset_millis: Arc::new(AtomicU64::new(0)) }
  }

  fn now(&self) -> Instant {
    self.base + Duration::from_millis(self.offset_millis.load(Ordering::SeqCst))
  }

  fn advance(&self, duration: Duration) {
    self.offset_millis.fetch_add(duration.as_millis() as u64, Ordering::SeqCst);
  }
}

impl CircuitBreakerShared {
  fn new_with_clock(
    max_failures: u32,
    reset_timeout: Duration,
    clock: impl Fn() -> Instant + Send + Sync + 'static,
  ) -> Self {
    Self {
      inner: ArcShared::new(RuntimeMutex::new(CircuitBreaker::new_with_clock(max_failures, reset_timeout, clock))),
    }
  }
}

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

  // cb1 を介した変更は cb2 から見える
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
  let clock = FakeClock::new();
  let clock_fn = {
    let clock = clock.clone();
    move || clock.now()
  };
  let cb = CircuitBreakerShared::new_with_clock(1, Duration::from_millis(10), clock_fn);

  let _ = cb.call(|| async { Err::<(), _>("fail") }).await;
  assert_eq!(cb.state(), CircuitBreakerState::Open);

  clock.advance(Duration::from_millis(20));

  let result = cb.call(|| async { Ok::<_, &str>(99) }).await;
  match result {
    | Ok(value) => assert_eq!(value, 99),
    | Err(err) => panic!("期待値: Ok(99), 実際: Err({err:?})"),
  }
  assert_eq!(cb.state(), CircuitBreakerState::Closed);
}

#[tokio::test]
async fn half_open_failure_reopens() {
  let clock = FakeClock::new();
  let clock_fn = {
    let clock = clock.clone();
    move || clock.now()
  };
  let cb = CircuitBreakerShared::new_with_clock(1, Duration::from_millis(10), clock_fn);

  let _ = cb.call(|| async { Err::<(), _>("fail") }).await;

  clock.advance(Duration::from_millis(20));

  let result = cb.call(|| async { Err::<(), _>("still broken") }).await;
  assert!(matches!(result, Err(CircuitBreakerCallError::Failed("still broken"))));
  assert_eq!(cb.state(), CircuitBreakerState::Open);
}

#[tokio::test]
async fn open_error_contains_remaining_duration() {
  let clock = FakeClock::new();
  let clock_fn = {
    let clock = clock.clone();
    move || clock.now()
  };
  let cb = CircuitBreakerShared::new_with_clock(1, Duration::from_secs(10), clock_fn);

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

#[tokio::test(start_paused = true)]
async fn cancel_during_half_open_records_failure() {
  let clock = FakeClock::new();
  let clock_fn = {
    let clock = clock.clone();
    move || clock.now()
  };
  let cb = CircuitBreakerShared::new_with_clock(1, Duration::from_millis(10), clock_fn);

  let _ = cb.call(|| async { Err::<(), _>("fail") }).await;
  assert_eq!(cb.state(), CircuitBreakerState::Open);

  clock.advance(Duration::from_millis(20));

  tokio::select! {
    biased;
    _ = tokio::task::yield_now() => {},
    _ = cb.call(|| async {
      std::future::pending::<()>().await;
      Ok::<_, &str>(42)
    }) => unreachable!(),
  }

  assert_eq!(cb.state(), CircuitBreakerState::Open);
}

/// Regression: Successful calls must not leak internal state.
#[tokio::test]
async fn successful_calls_do_not_leak_guard_resources() {
  let cb = CircuitBreakerShared::new(3, Duration::from_millis(100));

  for _ in 0..100 {
    let result = cb.call(|| async { Ok::<_, &str>(1) }).await;
    assert!(result.is_ok());
  }
  assert_eq!(cb.state(), CircuitBreakerState::Closed);
  assert_eq!(cb.failure_count(), 0);

  let _ = cb.call(|| async { Err::<(), _>("fail") }).await;
  assert_eq!(cb.failure_count(), 1);

  let result = cb.call(|| async { Ok::<_, &str>(42) }).await;
  match result {
    | Ok(value) => assert_eq!(value, 42),
    | Err(err) => panic!("expected Ok(42), got Err({err:?})"),
  }
  assert_eq!(cb.failure_count(), 0);
}
