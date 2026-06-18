//! Inner circuit breaker state machine.
//!
//! This type holds the mutable state for the three-state circuit breaker
//! (Closed → Open → HalfOpen) and exposes `&mut self` methods.
//! For a thread-safe, clonable wrapper see
//! [`CircuitBreakerShared`](super::CircuitBreakerShared).

use alloc::{boxed::Box, vec::Vec};
use core::{
  fmt::{Debug, Formatter, Result as FmtResult},
  time::Duration,
};

use super::{
  circuit_breaker_open_error::CircuitBreakerOpenError, circuit_breaker_state::CircuitBreakerState, clock::Clock,
};

#[cfg(test)]
#[path = "circuit_breaker_test.rs"]
mod tests;

type CircuitBreakerListener = dyn FnMut() + Send + 'static;

/// Three-state circuit breaker (Closed / Open / HalfOpen).
///
/// # Type parameters
///
/// * `C` — a [`Clock`] implementation that provides the current time.
///
/// # State transitions
///
/// * **Closed → Open** — when the consecutive failure count reaches `max_failures`.
/// * **Open → HalfOpen** — when `reset_timeout` has elapsed since the circuit opened.
/// * **HalfOpen → Closed** — when a probe call succeeds.
/// * **HalfOpen → Open** — when a probe call fails.
pub struct CircuitBreaker<C: Clock> {
  max_failures: u32,
  reset_timeout: Duration,
  open_reset_timeout: Duration,
  next_reset_timeout: Duration,
  max_reset_timeout: Duration,
  exponential_backoff_factor: f64,
  random_factor: f64,
  jitter_seed: u64,
  state: CircuitBreakerState,
  failure_count: u32,
  opened_at: Option<C::Instant>,
  clock: C,
  open_listeners: Vec<Box<CircuitBreakerListener>>,
  half_open_listeners: Vec<Box<CircuitBreakerListener>>,
  close_listeners: Vec<Box<CircuitBreakerListener>>,
}

impl<C: Clock> Debug for CircuitBreaker<C> {
  fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
    f.debug_struct("CircuitBreaker")
      .field("max_failures", &self.max_failures)
      .field("reset_timeout", &self.reset_timeout)
      .field("open_reset_timeout", &self.open_reset_timeout)
      .field("next_reset_timeout", &self.next_reset_timeout)
      .field("max_reset_timeout", &self.max_reset_timeout)
      .field("state", &self.state)
      .field("failure_count", &self.failure_count)
      .finish_non_exhaustive()
  }
}

impl<C: Clock> CircuitBreaker<C> {
  /// Creates a new circuit breaker in the **Closed** state with a custom
  /// clock.
  ///
  /// * `max_failures` — number of consecutive failures before the circuit trips. Must be greater
  ///   than zero.
  /// * `reset_timeout` — how long to wait in the **Open** state before allowing a probe call.
  /// * `clock` — a [`Clock`] implementation for obtaining the current time.
  ///
  /// # Panics
  ///
  /// Panics if `max_failures` is zero.
  #[must_use]
  pub fn new_with_clock(max_failures: u32, reset_timeout: Duration, clock: C) -> Self {
    assert!(max_failures > 0, "max_failures must be greater than zero");
    Self {
      max_failures,
      reset_timeout,
      open_reset_timeout: reset_timeout,
      next_reset_timeout: reset_timeout,
      max_reset_timeout: Duration::MAX,
      exponential_backoff_factor: 1.0,
      random_factor: 0.0,
      jitter_seed: 0x9e37_79b9_7f4a_7c15,
      clock,
      state: CircuitBreakerState::Closed,
      failure_count: 0,
      opened_at: None,
      open_listeners: Vec::new(),
      half_open_listeners: Vec::new(),
      close_listeners: Vec::new(),
    }
  }

  /// Returns a copy configured to exponentially increase the open reset timeout.
  ///
  /// The multiplier is `2.0`, matching Pekko `withExponentialBackoff`.
  #[must_use]
  pub const fn with_exponential_backoff(mut self, max_reset_timeout: Duration) -> Self {
    self.max_reset_timeout = max_reset_timeout;
    self.exponential_backoff_factor = 2.0;
    self
  }

  /// Returns a copy configured with additional random jitter for reset backoff.
  ///
  /// # Panics
  ///
  /// Panics when `random_factor` is outside `0.0..=1.0` or NaN.
  #[must_use]
  pub fn with_random_factor(mut self, random_factor: f64) -> Self {
    assert!((0.0..=1.0).contains(&random_factor) && !random_factor.is_nan(), "random_factor must be in [0.0, 1.0]");
    self.random_factor = random_factor;
    self
  }

  /// Adds a listener invoked when the breaker enters the open state.
  pub fn on_open<F>(&mut self, listener: F)
  where
    F: FnMut() + Send + 'static, {
    self.open_listeners.push(Box::new(listener));
  }

  /// Adds a listener invoked when the breaker enters the half-open state.
  pub fn on_half_open<F>(&mut self, listener: F)
  where
    F: FnMut() + Send + 'static, {
    self.half_open_listeners.push(Box::new(listener));
  }

  /// Adds a listener invoked when the breaker enters the closed state.
  pub fn on_close<F>(&mut self, listener: F)
  where
    F: FnMut() + Send + 'static, {
    self.close_listeners.push(Box::new(listener));
  }

  /// Returns the current state of the circuit breaker.
  #[must_use]
  pub const fn state(&self) -> CircuitBreakerState {
    self.state
  }

  /// Returns the current consecutive failure count.
  #[must_use]
  pub const fn failure_count(&self) -> u32 {
    self.failure_count
  }

  /// Returns the configured maximum failure threshold.
  #[must_use]
  pub const fn max_failures(&self) -> u32 {
    self.max_failures
  }

  /// Returns the configured reset timeout.
  #[must_use]
  pub const fn reset_timeout(&self) -> Duration {
    self.reset_timeout
  }

  /// Returns the configured upper bound for exponential reset timeout backoff.
  #[must_use]
  pub const fn max_reset_timeout(&self) -> Duration {
    self.max_reset_timeout
  }

  /// Returns the configured reset timeout backoff multiplier.
  #[must_use]
  pub const fn exponential_backoff_factor(&self) -> f64 {
    self.exponential_backoff_factor
  }

  /// Returns the configured random jitter factor for reset timeout backoff.
  #[must_use]
  pub const fn random_factor(&self) -> f64 {
    self.random_factor
  }

  /// Checks whether a call is currently permitted.
  ///
  /// * **Closed** — always permitted.
  /// * **Open** — permitted only when `reset_timeout` has elapsed (transitions to HalfOpen).
  /// * **HalfOpen** — permitted once for the probe call; subsequent calls are rejected.
  ///
  /// # Errors
  ///
  /// Returns [`CircuitBreakerOpenError`] with the remaining duration when the
  /// circuit is open and the reset timeout has not yet elapsed, or when a probe
  /// call is already in progress during the HalfOpen state.
  pub fn is_call_permitted(&mut self) -> Result<(), CircuitBreakerOpenError> {
    match self.state {
      | CircuitBreakerState::Closed => Ok(()),
      | CircuitBreakerState::Open => {
        let opened_at = self.opened_at_or_now();
        let elapsed = self.clock.elapsed_since(opened_at);
        if elapsed >= self.open_reset_timeout {
          // HalfOpen に遷移してプローブ呼び出しを許可する
          self.transition_to_half_open();
          Ok(())
        } else {
          Err(CircuitBreakerOpenError::new(self.open_reset_timeout - elapsed))
        }
      },
      | CircuitBreakerState::HalfOpen => {
        // HalfOpen には Open → HalfOpen 遷移でのみ到達し、その時点で
        // half_open_attempted = true が設定済み。プローブは1回限りなので
        // 後続の呼び出しはすべて拒否する。
        let remaining = self.remaining_in_open();
        Err(CircuitBreakerOpenError::new(remaining))
      },
    }
  }

  /// Records a successful call, transitioning to **Closed** if in **HalfOpen**.
  pub fn record_success(&mut self) {
    match self.state {
      | CircuitBreakerState::HalfOpen => {
        self.transition_to_closed();
      },
      | CircuitBreakerState::Closed => {
        // 成功時に連続失敗カウントをリセットする
        self.failure_count = 0;
      },
      | CircuitBreakerState::Open => {},
    }
  }

  /// Records a failed call, potentially transitioning to **Open**.
  pub fn record_failure(&mut self) {
    match self.state {
      | CircuitBreakerState::Closed => {
        self.failure_count += 1;
        if self.failure_count >= self.max_failures {
          self.transition_to_open();
        }
      },
      | CircuitBreakerState::HalfOpen => {
        self.transition_to_open();
      },
      | CircuitBreakerState::Open => {},
    }
  }

  /// Records a successful call, aware of whether this call was a HalfOpen probe.
  ///
  /// When `was_half_open` is true, the call was the probe — only then does success
  /// transition HalfOpen → Closed. Non-probe calls that complete after a state
  /// change are handled safely without corrupting the state machine.
  pub(crate) fn record_success_for(&mut self, was_half_open: bool) {
    if was_half_open {
      // probe 成功 — 現在の状態が HalfOpen なら Closed に遷移
      if self.state == CircuitBreakerState::HalfOpen {
        self.transition_to_closed();
      }
    } else {
      // 通常の呼び出し — Closed の場合のみ失敗カウントをリセット
      if self.state == CircuitBreakerState::Closed {
        self.failure_count = 0;
      }
    }
  }

  /// Records a failed call, aware of whether this call was a HalfOpen probe.
  ///
  /// When `was_half_open` is true, failure re-opens the circuit. Non-probe calls
  /// that complete after a state change only affect Closed state.
  pub(crate) fn record_failure_for(&mut self, was_half_open: bool) {
    if was_half_open {
      // probe 失敗 — HalfOpen なら Open に戻す
      if self.state == CircuitBreakerState::HalfOpen {
        self.transition_to_open();
      }
    } else {
      // 通常の呼び出し — Closed の場合のみ失敗カウントを増やす
      if self.state == CircuitBreakerState::Closed {
        self.failure_count += 1;
        if self.failure_count >= self.max_failures {
          self.transition_to_open();
        }
      }
    }
  }

  fn transition_to_open(&mut self) {
    self.state = CircuitBreakerState::Open;
    self.open_reset_timeout = self.next_reset_timeout;
    self.opened_at = Some(self.clock.now());
    self.notify_open();
    self.advance_next_reset_timeout();
  }

  fn transition_to_closed(&mut self) {
    self.state = CircuitBreakerState::Closed;
    self.failure_count = 0;
    self.opened_at = None;
    self.open_reset_timeout = self.reset_timeout;
    self.next_reset_timeout = self.reset_timeout;
    self.notify_close();
  }

  fn opened_at_or_now(&self) -> C::Instant {
    self.opened_at.unwrap_or_else(|| self.clock.now())
  }

  fn remaining_in_open(&self) -> Duration {
    self.opened_at.map_or(Duration::ZERO, |at| self.open_reset_timeout.saturating_sub(self.clock.elapsed_since(at)))
  }

  fn transition_to_half_open(&mut self) {
    self.state = CircuitBreakerState::HalfOpen;
    self.notify_half_open();
  }

  fn notify_open(&mut self) {
    for listener in &mut self.open_listeners {
      listener();
    }
  }

  fn notify_half_open(&mut self) {
    for listener in &mut self.half_open_listeners {
      listener();
    }
  }

  fn notify_close(&mut self) {
    for listener in &mut self.close_listeners {
      listener();
    }
  }

  fn advance_next_reset_timeout(&mut self) {
    let multiplier = self.exponential_backoff_factor * self.next_jitter_multiplier();
    let next = Self::mul_duration_capped(self.open_reset_timeout, multiplier, self.max_reset_timeout);
    self.next_reset_timeout = next;
  }

  fn next_jitter_multiplier(&mut self) -> f64 {
    if self.random_factor == 0.0 {
      return 1.0;
    }
    let mut x = self.jitter_seed;
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    self.jitter_seed = x;
    1.0 + (x as f64 / u64::MAX as f64) * self.random_factor
  }

  fn mul_duration_capped(duration: Duration, factor: f64, max: Duration) -> Duration {
    let scaled = duration.as_nanos() as f64 * factor;
    if !scaled.is_finite() || scaled >= max.as_nanos() as f64 {
      return max;
    }
    let truncated = scaled as u128;
    let nanos = if scaled > truncated as f64 { truncated.saturating_add(1) } else { truncated };
    Self::duration_from_nanos_capped(nanos).min(max)
  }

  const fn duration_from_nanos_capped(nanos: u128) -> Duration {
    const NANOS_PER_SEC: u128 = 1_000_000_000;
    let secs = nanos / NANOS_PER_SEC;
    let sub_nanos = (nanos % NANOS_PER_SEC) as u32;
    if secs > u64::MAX as u128 { Duration::MAX } else { Duration::new(secs as u64, sub_nanos) }
  }
}
