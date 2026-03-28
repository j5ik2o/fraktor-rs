use core::{
  sync::atomic::{AtomicU64, Ordering},
  time::Duration,
};

use fraktor_utils_rs::core::sync::SharedAccess;

use crate::core::kernel::pattern::{CircuitBreakerCallError, CircuitBreakerShared, CircuitBreakerState, Clock};

/// A deterministic clock for unit tests that does not depend on `std::time`.
#[derive(Clone)]
struct FakeClock {
  offset_millis: alloc::sync::Arc<AtomicU64>,
}

impl FakeClock {
  fn new() -> Self {
    Self { offset_millis: alloc::sync::Arc::new(AtomicU64::new(0)) }
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

#[test]
fn new_starts_closed() {
  let cb = CircuitBreakerShared::new_with_clock(3, Duration::from_millis(100), FakeClock::new());
  assert_eq!(cb.state(), CircuitBreakerState::Closed);
  assert_eq!(cb.failure_count(), 0);
}

#[test]
fn clone_shares_state() {
  let cb1 = CircuitBreakerShared::new_with_clock(3, Duration::from_millis(100), FakeClock::new());
  let cb2 = cb1.clone();

  SharedAccess::with_write(&cb1, |inner| {
    inner.record_failure();
  });
  assert_eq!(cb2.failure_count(), 1);
}

#[tokio::test]
async fn call_succeeds_in_closed() {
  let cb = CircuitBreakerShared::new_with_clock(3, Duration::from_millis(100), FakeClock::new());

  let result = cb.call(|| async { Ok::<_, &str>(42) }).await;
  match result {
    | Ok(value) => assert_eq!(value, 42),
    | Err(err) => panic!("期待値: Ok(42), 実際: Err({err:?})"),
  }
  assert_eq!(cb.state(), CircuitBreakerState::Closed);
}

#[tokio::test]
async fn call_records_failure() {
  let cb = CircuitBreakerShared::new_with_clock(2, Duration::from_millis(100), FakeClock::new());

  let result = cb.call(|| async { Err::<(), _>("oops") }).await;
  assert!(matches!(result, Err(CircuitBreakerCallError::Failed("oops"))));
  assert_eq!(cb.failure_count(), 1);
}

#[tokio::test]
async fn call_trips_after_max_failures() {
  let cb = CircuitBreakerShared::new_with_clock(2, Duration::from_millis(100), FakeClock::new());

  let _ = cb.call(|| async { Err::<(), _>("a") }).await;
  let _ = cb.call(|| async { Err::<(), _>("b") }).await;
  assert_eq!(cb.state(), CircuitBreakerState::Open);

  let result = cb.call(|| async { Ok::<_, &str>(1) }).await;
  assert!(matches!(result, Err(CircuitBreakerCallError::Open(_))));
}

#[tokio::test]
async fn call_recovers_after_reset_timeout() {
  let clock = FakeClock::new();
  let cb = CircuitBreakerShared::new_with_clock(1, Duration::from_millis(10), clock.clone());

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
  let cb = CircuitBreakerShared::new_with_clock(1, Duration::from_millis(10), clock.clone());

  let _ = cb.call(|| async { Err::<(), _>("fail") }).await;

  clock.advance(Duration::from_millis(20));

  let result = cb.call(|| async { Err::<(), _>("still broken") }).await;
  assert!(matches!(result, Err(CircuitBreakerCallError::Failed("still broken"))));
  assert_eq!(cb.state(), CircuitBreakerState::Open);
}
