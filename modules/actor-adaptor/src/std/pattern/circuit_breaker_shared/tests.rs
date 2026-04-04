extern crate std;

use core::{
  sync::atomic::{AtomicU64, Ordering},
  time::Duration,
};
use std::{sync::Arc, time::Instant};

use fraktor_actor_rs::core::kernel::pattern::{CircuitBreakerCallError, CircuitBreakerState, Clock};
use fraktor_utils_rs::core::sync::SharedAccess;

use crate::std::pattern::circuit_breaker_shared;

#[derive(Clone)]
struct FakeClock {
  base:          Instant,
  offset_millis: Arc<AtomicU64>,
}

impl FakeClock {
  fn new() -> Self {
    Self { base: Instant::now(), offset_millis: Arc::new(AtomicU64::new(0)) }
  }

  fn advance(&self, duration: Duration) {
    self.offset_millis.fetch_add(duration.as_millis() as u64, Ordering::SeqCst);
  }
}

impl Clock for FakeClock {
  type Instant = Instant;

  fn now(&self) -> Self::Instant {
    self.base + Duration::from_millis(self.offset_millis.load(Ordering::SeqCst))
  }

  fn elapsed_since(&self, earlier: Self::Instant) -> Duration {
    self.now().duration_since(earlier)
  }
}

#[test]
fn new_starts_closed() {
  let cb = circuit_breaker_shared(3, Duration::from_millis(100));
  assert_eq!(cb.state(), CircuitBreakerState::Closed);
  assert_eq!(cb.failure_count(), 0);
}

#[test]
fn clone_shares_state() {
  let cb1 = circuit_breaker_shared(3, Duration::from_millis(100));
  let cb2 = cb1.clone();

  // cb1 を介した変更は cb2 から見える
  SharedAccess::with_write(&cb1, |inner| {
    inner.record_failure();
  });
  assert_eq!(cb2.failure_count(), 1);
}

#[tokio::test]
async fn call_succeeds_in_closed() {
  let cb = circuit_breaker_shared(3, Duration::from_millis(100));

  let result = cb.call(|| async { Ok::<_, &str>(42) }).await;
  match result {
    | Ok(value) => assert_eq!(value, 42),
    | Err(err) => panic!("期待値: Ok(42), 実際: Err({err:?})"),
  }
  assert_eq!(cb.state(), CircuitBreakerState::Closed);
}

#[tokio::test]
async fn call_records_failure() {
  let cb = circuit_breaker_shared(2, Duration::from_millis(100));

  let result = cb.call(|| async { Err::<(), _>("oops") }).await;
  assert!(matches!(result, Err(CircuitBreakerCallError::Failed("oops"))));
  assert_eq!(cb.failure_count(), 1);
}

#[tokio::test]
async fn call_trips_after_max_failures() {
  let cb = circuit_breaker_shared(2, Duration::from_millis(100));

  let _ = cb.call(|| async { Err::<(), _>("a") }).await;
  let _ = cb.call(|| async { Err::<(), _>("b") }).await;
  assert_eq!(cb.state(), CircuitBreakerState::Open);

  let result = cb.call(|| async { Ok::<_, &str>(1) }).await;
  assert!(matches!(result, Err(CircuitBreakerCallError::Open(_))));
}

// ---- FakeClock を使うテスト ----
// FakeClock<Instant> は StdClock とは異なる Clock 実装のため、std の型エイリアス
// `CircuitBreakerShared`（= CircuitBreakerShared<StdClock>）は使用できない。
// core パスを直接参照するのは意図的であり、std 公開面の回帰は上記テストでカバーする。

#[tokio::test]
async fn call_recovers_after_reset_timeout() {
  let clock = FakeClock::new();
  let cb = fraktor_actor_rs::core::kernel::pattern::CircuitBreakerShared::new_with_clock(
    1,
    Duration::from_millis(10),
    clock.clone(),
  );

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
  let cb = fraktor_actor_rs::core::kernel::pattern::CircuitBreakerShared::new_with_clock(
    1,
    Duration::from_millis(10),
    clock.clone(),
  );

  let _ = cb.call(|| async { Err::<(), _>("fail") }).await;

  clock.advance(Duration::from_millis(20));

  let result = cb.call(|| async { Err::<(), _>("still broken") }).await;
  assert!(matches!(result, Err(CircuitBreakerCallError::Failed("still broken"))));
  assert_eq!(cb.state(), CircuitBreakerState::Open);
}

#[tokio::test]
async fn open_error_contains_remaining_duration() {
  let clock = FakeClock::new();
  let cb = fraktor_actor_rs::core::kernel::pattern::CircuitBreakerShared::new_with_clock(
    1,
    Duration::from_secs(10),
    clock.clone(),
  );

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
  let cb = fraktor_actor_rs::core::kernel::pattern::CircuitBreakerShared::new_with_clock(
    1,
    Duration::from_millis(10),
    clock.clone(),
  );

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
  let cb = circuit_breaker_shared(3, Duration::from_millis(100));

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
