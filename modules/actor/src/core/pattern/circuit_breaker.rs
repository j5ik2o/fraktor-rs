//! Inner circuit breaker state machine.
//!
//! This type holds the mutable state for the three-state circuit breaker
//! (Closed → Open → HalfOpen) and exposes `&mut self` methods.
//! For a thread-safe, clonable wrapper see
//! [`CircuitBreakerShared`](super::CircuitBreakerShared).

use core::{fmt, time::Duration};

use super::{
  circuit_breaker_open_error::CircuitBreakerOpenError, circuit_breaker_state::CircuitBreakerState, clock::Clock,
};

#[cfg(test)]
mod tests;

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
  max_failures:        u32,
  reset_timeout:       Duration,
  state:               CircuitBreakerState,
  failure_count:       u32,
  opened_at:           Option<C::Instant>,
  half_open_attempted: bool,
  clock:               C,
}

impl<C: Clock> fmt::Debug for CircuitBreaker<C> {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("CircuitBreaker")
      .field("max_failures", &self.max_failures)
      .field("reset_timeout", &self.reset_timeout)
      .field("state", &self.state)
      .field("failure_count", &self.failure_count)
      .field("half_open_attempted", &self.half_open_attempted)
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
      clock,
      state: CircuitBreakerState::Closed,
      failure_count: 0,
      opened_at: None,
      half_open_attempted: false,
    }
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
        if elapsed >= self.reset_timeout {
          // HalfOpen に遷移してプローブ呼び出しを許可する
          self.state = CircuitBreakerState::HalfOpen;
          self.half_open_attempted = true;
          Ok(())
        } else {
          Err(CircuitBreakerOpenError::new(self.reset_timeout - elapsed))
        }
      },
      | CircuitBreakerState::HalfOpen => {
        if self.half_open_attempted {
          // プローブ呼び出しが既に進行中 — 拒否する
          let remaining = self.remaining_in_open();
          Err(CircuitBreakerOpenError::new(remaining))
        } else {
          self.half_open_attempted = true;
          Ok(())
        }
      },
    }
  }

  /// Records a successful call, transitioning to **Closed** if in **HalfOpen**.
  pub const fn record_success(&mut self) {
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

  fn transition_to_open(&mut self) {
    self.state = CircuitBreakerState::Open;
    self.opened_at = Some(self.clock.now());
    self.half_open_attempted = false;
  }

  const fn transition_to_closed(&mut self) {
    self.state = CircuitBreakerState::Closed;
    self.failure_count = 0;
    self.opened_at = None;
    self.half_open_attempted = false;
  }

  fn opened_at_or_now(&self) -> C::Instant {
    self.opened_at.unwrap_or_else(|| self.clock.now())
  }

  fn remaining_in_open(&self) -> Duration {
    self.opened_at.map_or(Duration::ZERO, |at| self.reset_timeout.saturating_sub(self.clock.elapsed_since(at)))
  }
}
