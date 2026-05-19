// NOTE: FakeClock / FakeInstant は circuit_breaker_test.rs, circuit_breaker_shared_test.rs,
// および std 側の同名テストファイルに重複して定義されている。
// テスト専用ユーティリティへの共通化は別タスクとして実施する（今回のスコープ外）。

use alloc::sync::Arc;
use core::{
  sync::atomic::{AtomicU64, Ordering},
  time::Duration,
};

use super::CircuitBreaker;
use crate::pattern::{CircuitBreakerState, Clock};

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

#[test]
#[should_panic(expected = "max_failures must be greater than zero")]
fn rejects_zero_max_failures() {
  drop(CircuitBreaker::new_with_clock(0, Duration::from_millis(100), FakeClock::new()));
}

#[test]
fn new_starts_in_closed_state() {
  let cb = CircuitBreaker::new_with_clock(3, Duration::from_millis(100), FakeClock::new());
  assert_eq!(cb.state(), CircuitBreakerState::Closed);
  assert_eq!(cb.failure_count(), 0);
  assert_eq!(cb.max_failures(), 3);
  assert_eq!(cb.reset_timeout(), Duration::from_millis(100));
}

#[test]
fn stays_closed_on_success() {
  let mut cb = CircuitBreaker::new_with_clock(3, Duration::from_millis(100), FakeClock::new());
  cb.record_success();
  assert_eq!(cb.state(), CircuitBreakerState::Closed);
  assert_eq!(cb.failure_count(), 0);
}

#[test]
fn stays_closed_when_failures_below_threshold() {
  let mut cb = CircuitBreaker::new_with_clock(3, Duration::from_millis(100), FakeClock::new());
  cb.record_failure();
  assert_eq!(cb.failure_count(), 1);
  assert_eq!(cb.state(), CircuitBreakerState::Closed);

  cb.record_failure();
  assert_eq!(cb.failure_count(), 2);
  assert_eq!(cb.state(), CircuitBreakerState::Closed);
}

#[test]
fn transitions_to_open_at_max_failures() {
  let mut cb = CircuitBreaker::new_with_clock(3, Duration::from_millis(100), FakeClock::new());
  cb.record_failure();
  cb.record_failure();
  cb.record_failure();
  assert_eq!(cb.state(), CircuitBreakerState::Open);
}

#[test]
fn resets_failure_count_on_success_in_closed() {
  let mut cb = CircuitBreaker::new_with_clock(3, Duration::from_millis(100), FakeClock::new());
  cb.record_failure();
  cb.record_failure();
  assert_eq!(cb.failure_count(), 2);

  cb.record_success();
  assert_eq!(cb.failure_count(), 0);
  assert_eq!(cb.state(), CircuitBreakerState::Closed);
}

#[test]
fn open_rejects_calls() {
  let clock = FakeClock::new();
  let mut cb = CircuitBreaker::new_with_clock(1, Duration::from_secs(10), clock);
  cb.record_failure();
  assert_eq!(cb.state(), CircuitBreakerState::Open);

  let Err(err) = cb.is_call_permitted() else {
    panic!("expected Err but got Ok(())");
  };
  assert!(err.remaining() > Duration::ZERO);
}

#[test]
fn open_transitions_to_half_open_after_reset_timeout() {
  let clock = FakeClock::new();
  let mut cb = CircuitBreaker::new_with_clock(1, Duration::from_millis(10), clock.clone());
  cb.record_failure();
  assert_eq!(cb.state(), CircuitBreakerState::Open);

  clock.advance(Duration::from_millis(20));

  assert!(cb.is_call_permitted().is_ok());
  assert_eq!(cb.state(), CircuitBreakerState::HalfOpen);
}

#[test]
fn open_remains_open_before_reset_timeout() {
  let clock = FakeClock::new();
  let mut cb = CircuitBreaker::new_with_clock(1, Duration::from_millis(100), clock.clone());
  cb.record_failure();
  assert_eq!(cb.state(), CircuitBreakerState::Open);

  clock.advance(Duration::from_millis(50));

  assert!(cb.is_call_permitted().is_err());
  assert_eq!(cb.state(), CircuitBreakerState::Open);
}

#[test]
fn half_open_rejects_second_call() {
  let clock = FakeClock::new();
  let mut cb = CircuitBreaker::new_with_clock(1, Duration::from_millis(10), clock.clone());
  cb.record_failure();
  clock.advance(Duration::from_millis(20));

  assert!(cb.is_call_permitted().is_ok());
  assert_eq!(cb.state(), CircuitBreakerState::HalfOpen);

  assert!(cb.is_call_permitted().is_err());
}

#[test]
fn half_open_success_transitions_to_closed() {
  let clock = FakeClock::new();
  let mut cb = CircuitBreaker::new_with_clock(1, Duration::from_millis(10), clock.clone());
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
  let mut cb = CircuitBreaker::new_with_clock(1, Duration::from_millis(10), clock.clone());
  cb.record_failure();
  clock.advance(Duration::from_millis(20));
  assert!(cb.is_call_permitted().is_ok());

  cb.record_failure();

  assert_eq!(cb.state(), CircuitBreakerState::Open);
}

#[test]
fn closed_always_permits() {
  let mut cb = CircuitBreaker::new_with_clock(5, Duration::from_secs(1), FakeClock::new());
  for _ in 0..10 {
    assert!(cb.is_call_permitted().is_ok());
    cb.record_success();
  }
  assert_eq!(cb.state(), CircuitBreakerState::Closed);
}

#[test]
fn max_failures_one_trips_on_first_failure() {
  let mut cb = CircuitBreaker::new_with_clock(1, Duration::from_millis(100), FakeClock::new());
  cb.record_failure();
  assert_eq!(cb.state(), CircuitBreakerState::Open);
}

#[test]
fn recovery_cycle_closed_open_half_open_closed() {
  let clock = FakeClock::new();
  let mut cb = CircuitBreaker::new_with_clock(2, Duration::from_millis(10), clock.clone());

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
  let mut cb = CircuitBreaker::new_with_clock(1, Duration::from_millis(100), clock.clone());
  cb.record_failure();

  clock.advance(Duration::from_millis(30));
  let err = cb.is_call_permitted().unwrap_err();

  assert_eq!(err.remaining(), Duration::from_millis(70));
}
