use alloc::sync::Arc;
use core::{
  sync::atomic::{AtomicU64, Ordering},
  time::Duration,
};

use fraktor_utils_core_rs::sync::SharedAccess;

use crate::pattern::{CircuitBreaker, CircuitBreakerCallError, CircuitBreakerShared, CircuitBreakerState, Clock};

/// A deterministic clock for unit tests that does not depend on `std::time`.
#[derive(Clone)]
struct FakeClock {
  offset_millis: Arc<AtomicU64>,
}

impl FakeClock {
  fn new() -> Self {
    Self { offset_millis: Arc::new(AtomicU64::new(0)) }
  }

  fn advance(&self, duration: Duration) {
    self.offset_millis.fetch_add(duration.as_millis() as u64, Ordering::SeqCst);
  }
}

/// A simple `Copy + Ord` instant represented as milliseconds since epoch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct FakeInstant(u64);

impl Clock for FakeClock {
  type Instant = FakeInstant;

  fn now(&self) -> Self::Instant {
    FakeInstant(self.offset_millis.load(Ordering::SeqCst))
  }

  fn elapsed_since(&self, earlier: Self::Instant) -> Duration {
    let now = self.offset_millis.load(Ordering::SeqCst);
    Duration::from_millis(now.saturating_sub(earlier.0))
  }
}

fn shared_with_clock(max_failures: u32, reset_timeout: Duration, clock: FakeClock) -> CircuitBreakerShared<FakeClock> {
  CircuitBreakerShared::new(CircuitBreaker::new_with_clock(max_failures, reset_timeout, clock))
}

#[test]
fn new_starts_closed() {
  let cb = shared_with_clock(3, Duration::from_millis(100), FakeClock::new());
  assert_eq!(cb.state(), CircuitBreakerState::Closed);
  assert_eq!(cb.failure_count(), 0);
}

#[test]
fn builtin_spin_shared_wraps_circuit_breaker() {
  let cb = CircuitBreakerShared::new(CircuitBreaker::new_with_clock(3, Duration::from_millis(100), FakeClock::new()));
  assert_eq!(cb.state(), CircuitBreakerState::Closed);
  assert_eq!(cb.failure_count(), 0);
}

#[test]
fn clone_shares_state() {
  let cb1 = shared_with_clock(3, Duration::from_millis(100), FakeClock::new());
  let cb2 = cb1.clone();

  SharedAccess::with_write(&cb1, |inner| {
    inner.record_failure();
  });
  assert_eq!(cb2.failure_count(), 1);
}

#[tokio::test]
async fn call_succeeds_in_closed() {
  let cb = shared_with_clock(3, Duration::from_millis(100), FakeClock::new());

  let result = cb.call(|| async { Ok::<_, &str>(42) }).await;
  match result {
    | Ok(value) => assert_eq!(value, 42),
    | Err(err) => panic!("expected Ok(42), got Err({err:?})"),
  }
  assert_eq!(cb.state(), CircuitBreakerState::Closed);
}

#[tokio::test]
async fn call_records_failure() {
  let cb = shared_with_clock(2, Duration::from_millis(100), FakeClock::new());

  let result = cb.call(|| async { Err::<(), _>("oops") }).await;
  assert!(matches!(result, Err(CircuitBreakerCallError::Failed("oops"))));
  assert_eq!(cb.failure_count(), 1);
}

#[tokio::test]
async fn call_trips_after_max_failures() {
  let cb = shared_with_clock(2, Duration::from_millis(100), FakeClock::new());

  let _ = cb.call(|| async { Err::<(), _>("a") }).await;
  let _ = cb.call(|| async { Err::<(), _>("b") }).await;
  assert_eq!(cb.state(), CircuitBreakerState::Open);

  let result = cb.call(|| async { Ok::<_, &str>(1) }).await;
  assert!(matches!(result, Err(CircuitBreakerCallError::Open(_))));
}

#[tokio::test]
async fn call_recovers_after_reset_timeout() {
  let clock = FakeClock::new();
  let cb = shared_with_clock(1, Duration::from_millis(10), clock.clone());

  let _ = cb.call(|| async { Err::<(), _>("fail") }).await;
  assert_eq!(cb.state(), CircuitBreakerState::Open);

  clock.advance(Duration::from_millis(20));

  let result = cb.call(|| async { Ok::<_, &str>(99) }).await;
  match result {
    | Ok(value) => assert_eq!(value, 99),
    | Err(err) => panic!("expected Ok(99), got Err({err:?})"),
  }
  assert_eq!(cb.state(), CircuitBreakerState::Closed);
}

#[tokio::test]
async fn half_open_failure_reopens() {
  let clock = FakeClock::new();
  let cb = shared_with_clock(1, Duration::from_millis(10), clock.clone());

  let _ = cb.call(|| async { Err::<(), _>("fail") }).await;

  clock.advance(Duration::from_millis(20));

  let result = cb.call(|| async { Err::<(), _>("still broken") }).await;
  assert!(matches!(result, Err(CircuitBreakerCallError::Failed("still broken"))));
  assert_eq!(cb.state(), CircuitBreakerState::Open);
}

#[tokio::test]
async fn open_error_contains_remaining_duration() {
  let clock = FakeClock::new();
  let cb = shared_with_clock(1, Duration::from_secs(10), clock.clone());

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
  let cb = shared_with_clock(1, Duration::from_millis(10), clock.clone());

  let _ = cb.call(|| async { Err::<(), _>("fail") }).await;
  assert_eq!(cb.state(), CircuitBreakerState::Open);

  clock.advance(Duration::from_millis(20));

  tokio::select! {
    biased;
    _ = tokio::task::yield_now() => {},
    _ = cb.call(|| async {
      tokio::time::sleep(Duration::from_secs(3600)).await;
      Ok::<_, &str>(42)
    }) => unreachable!(),
  }

  assert_eq!(cb.state(), CircuitBreakerState::Open);
}

#[tokio::test]
async fn successful_calls_do_not_leak_guard_resources() {
  let cb = shared_with_clock(3, Duration::from_millis(100), FakeClock::new());

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
