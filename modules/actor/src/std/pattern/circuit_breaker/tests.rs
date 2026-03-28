extern crate std;

use core::{
  sync::atomic::{AtomicU64, Ordering},
  time::Duration,
};
use std::{sync::Arc, time::Instant};

use crate::{
  core::kernel::pattern::{CircuitBreakerState, Clock},
  std::pattern::circuit_breaker,
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
#[should_panic(expected = "max_failures must be greater than zero")]
fn rejects_zero_max_failures() {
  let _ = circuit_breaker(0, Duration::from_millis(100));
}

#[test]
fn new_starts_in_closed_state() {
  let cb = circuit_breaker(3, Duration::from_millis(100));
  assert_eq!(cb.state(), CircuitBreakerState::Closed);
  assert_eq!(cb.failure_count(), 0);
  assert_eq!(cb.max_failures(), 3);
  assert_eq!(cb.reset_timeout(), Duration::from_millis(100));
}

#[test]
fn stays_closed_on_success() {
  let mut cb = circuit_breaker(3, Duration::from_millis(100));
  cb.record_success();
  assert_eq!(cb.state(), CircuitBreakerState::Closed);
  assert_eq!(cb.failure_count(), 0);
}

#[test]
fn stays_closed_when_failures_below_threshold() {
  let mut cb = circuit_breaker(3, Duration::from_millis(100));
  cb.record_failure();
  assert_eq!(cb.failure_count(), 1);
  assert_eq!(cb.state(), CircuitBreakerState::Closed);

  cb.record_failure();
  assert_eq!(cb.failure_count(), 2);
  assert_eq!(cb.state(), CircuitBreakerState::Closed);
}

#[test]
fn transitions_to_open_at_max_failures() {
  let mut cb = circuit_breaker(3, Duration::from_millis(100));
  cb.record_failure();
  cb.record_failure();
  cb.record_failure();
  assert_eq!(cb.state(), CircuitBreakerState::Open);
}

#[test]
fn resets_failure_count_on_success_in_closed() {
  let mut cb = circuit_breaker(3, Duration::from_millis(100));
  cb.record_failure();
  cb.record_failure();
  assert_eq!(cb.failure_count(), 2);

  cb.record_success();
  assert_eq!(cb.failure_count(), 0);
  assert_eq!(cb.state(), CircuitBreakerState::Closed);
}

// ---- FakeClock を使うテスト ----
// FakeClock<Instant> は StdClock とは異なる Clock 実装のため、std の型エイリアス
// `CircuitBreaker`（= CircuitBreaker<StdClock>）は使用できない。
// core パスを直接参照するのは意図的であり、std 公開面の回帰は上記テストでカバーする。

#[test]
fn open_rejects_calls() {
  let clock = FakeClock::new();
  let mut cb = crate::core::kernel::pattern::CircuitBreaker::new_with_clock(1, Duration::from_secs(10), clock);
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
  let clock = FakeClock::new();
  let mut cb =
    crate::core::kernel::pattern::CircuitBreaker::new_with_clock(1, Duration::from_millis(10), clock.clone());
  cb.record_failure();
  assert_eq!(cb.state(), CircuitBreakerState::Open);

  clock.advance(Duration::from_millis(20));

  assert!(cb.is_call_permitted().is_ok());
  assert_eq!(cb.state(), CircuitBreakerState::HalfOpen);
}

#[test]
fn open_remains_open_before_reset_timeout() {
  let clock = FakeClock::new();
  let mut cb =
    crate::core::kernel::pattern::CircuitBreaker::new_with_clock(1, Duration::from_millis(100), clock.clone());
  cb.record_failure();
  assert_eq!(cb.state(), CircuitBreakerState::Open);

  clock.advance(Duration::from_millis(50));

  assert!(cb.is_call_permitted().is_err());
  assert_eq!(cb.state(), CircuitBreakerState::Open);
}

#[test]
fn half_open_rejects_second_call() {
  let clock = FakeClock::new();
  let mut cb =
    crate::core::kernel::pattern::CircuitBreaker::new_with_clock(1, Duration::from_millis(10), clock.clone());
  cb.record_failure();
  clock.advance(Duration::from_millis(20));

  assert!(cb.is_call_permitted().is_ok());
  assert_eq!(cb.state(), CircuitBreakerState::HalfOpen);

  assert!(cb.is_call_permitted().is_err());
}

#[test]
fn half_open_success_transitions_to_closed() {
  let clock = FakeClock::new();
  let mut cb =
    crate::core::kernel::pattern::CircuitBreaker::new_with_clock(1, Duration::from_millis(10), clock.clone());
  cb.record_failure();
  clock.advance(Duration::from_millis(20));
  assert!(cb.is_call_permitted().is_ok());

  cb.record_success();

  assert_eq!(cb.state(), CircuitBreakerState::Closed);
  assert_eq!(cb.failure_count(), 0);
}

#[test]
fn half_open_failure_transitions_to_open() {
  let clock = FakeClock::new();
  let mut cb =
    crate::core::kernel::pattern::CircuitBreaker::new_with_clock(1, Duration::from_millis(10), clock.clone());
  cb.record_failure();
  clock.advance(Duration::from_millis(20));
  assert!(cb.is_call_permitted().is_ok());

  cb.record_failure();

  assert_eq!(cb.state(), CircuitBreakerState::Open);
}

#[test]
fn closed_always_permits() {
  let mut cb = circuit_breaker(5, Duration::from_secs(1));
  for _ in 0..10 {
    assert!(cb.is_call_permitted().is_ok());
    cb.record_success();
  }
  assert_eq!(cb.state(), CircuitBreakerState::Closed);
}

#[test]
fn max_failures_one_trips_on_first_failure() {
  let mut cb = circuit_breaker(1, Duration::from_millis(100));
  cb.record_failure();
  assert_eq!(cb.state(), CircuitBreakerState::Open);
}

#[test]
fn recovery_cycle_closed_open_half_open_closed() {
  let clock = FakeClock::new();
  let mut cb =
    crate::core::kernel::pattern::CircuitBreaker::new_with_clock(2, Duration::from_millis(10), clock.clone());

  // Closed → Open
  cb.record_failure();
  cb.record_failure();
  assert_eq!(cb.state(), CircuitBreakerState::Open);

  // Open → HalfOpen (advance fake clock)
  clock.advance(Duration::from_millis(20));
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

#[test]
fn open_error_reports_correct_remaining_duration() {
  let clock = FakeClock::new();
  let mut cb =
    crate::core::kernel::pattern::CircuitBreaker::new_with_clock(1, Duration::from_millis(100), clock.clone());
  cb.record_failure();

  clock.advance(Duration::from_millis(30));
  let err = cb.is_call_permitted().unwrap_err();

  assert!(err.remaining() <= Duration::from_millis(70));
  assert!(err.remaining() >= Duration::from_millis(69));
}
