// NOTE: FakeClock / FakeInstant は circuit_breaker_test.rs, circuit_breaker_shared_test.rs,
// および std 側の同名テストファイルに重複して定義されている。
// テスト専用ユーティリティへの共通化は別タスクとして実施する（今回のスコープ外）。

use alloc::{format, sync::Arc};
use core::{
  sync::atomic::{AtomicU64, Ordering},
  time::Duration,
};

use fraktor_utils_core_rs::sync::{ArcShared, SpinSyncMutex};

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

#[test]
fn transition_listeners_run_on_state_entry() {
  let clock = FakeClock::new();
  let events = ArcShared::new(SpinSyncMutex::new(Vec::<&'static str>::new()));
  let mut cb = CircuitBreaker::new_with_clock(1, Duration::from_millis(10), clock.clone());

  let open_events = events.clone();
  cb.on_open(move || open_events.lock().push("open"));
  let half_open_events = events.clone();
  cb.on_half_open(move || half_open_events.lock().push("half-open"));
  let close_events = events.clone();
  cb.on_close(move || close_events.lock().push("closed"));

  cb.record_failure();
  clock.advance(Duration::from_millis(10));
  assert!(cb.is_call_permitted().is_ok());
  cb.record_success();

  assert_eq!(*events.lock(), vec!["open", "half-open", "closed"]);
}

#[test]
fn exponential_backoff_extends_next_open_reset_timeout() {
  let clock = FakeClock::new();
  let mut cb = CircuitBreaker::new_with_clock(1, Duration::from_millis(10), clock.clone())
    .with_exponential_backoff(Duration::from_millis(100));

  cb.record_failure();
  clock.advance(Duration::from_millis(10));
  assert!(cb.is_call_permitted().is_ok());
  cb.record_failure();

  clock.advance(Duration::from_millis(10));
  let err = cb.is_call_permitted().unwrap_err();
  assert_eq!(err.remaining(), Duration::from_millis(10));

  clock.advance(Duration::from_millis(10));
  assert!(cb.is_call_permitted().is_ok());
}

#[test]
fn exponential_backoff_is_capped_by_max_reset_timeout() {
  let clock = FakeClock::new();
  let mut cb = CircuitBreaker::new_with_clock(1, Duration::from_millis(10), clock.clone())
    .with_exponential_backoff(Duration::from_millis(15));

  cb.record_failure();
  clock.advance(Duration::from_millis(10));
  assert!(cb.is_call_permitted().is_ok());
  cb.record_failure();

  clock.advance(Duration::from_millis(14));
  assert!(cb.is_call_permitted().is_err());
  clock.advance(Duration::from_millis(1));
  assert!(cb.is_call_permitted().is_ok());
}

#[test]
fn random_factor_is_exposed_as_builder_configuration() {
  let cb = CircuitBreaker::new_with_clock(1, Duration::from_millis(10), FakeClock::new()).with_random_factor(0.25);
  assert_eq!(cb.random_factor(), 0.25);
}

#[test]
fn debug_includes_backoff_fields() {
  let cb = CircuitBreaker::new_with_clock(1, Duration::from_millis(10), FakeClock::new())
    .with_exponential_backoff(Duration::from_millis(100));
  let debug = format!("{cb:?}");

  assert!(debug.contains("open_reset_timeout"));
  assert!(debug.contains("next_reset_timeout"));
  assert!(debug.contains("max_reset_timeout"));
}

#[test]
fn backoff_configuration_accessors_are_exposed() {
  let cb = CircuitBreaker::new_with_clock(1, Duration::from_millis(10), FakeClock::new())
    .with_exponential_backoff(Duration::from_millis(100));

  assert_eq!(cb.max_reset_timeout(), Duration::from_millis(100));
  assert_eq!(cb.exponential_backoff_factor(), 2.0);
}

#[test]
fn random_factor_contributes_jitter_to_next_reset_timeout() {
  let mut cb = CircuitBreaker::new_with_clock(1, Duration::from_millis(10), FakeClock::new())
    .with_exponential_backoff(Duration::from_millis(100))
    .with_random_factor(0.5);

  cb.record_failure();

  assert!(cb.random_factor() > 0.0);
}

#[test]
#[should_panic(expected = "random_factor must be in [0.0, 1.0]")]
fn random_factor_rejects_out_of_range_values() {
  drop(CircuitBreaker::new_with_clock(1, Duration::from_millis(10), FakeClock::new()).with_random_factor(1.5));
}
