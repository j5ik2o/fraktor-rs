//! Inner circuit breaker state machine.
//!
//! This type holds the mutable state for the three-state circuit breaker
//! (Closed → Open → HalfOpen) and exposes `&mut self` methods.
//! For a thread-safe, clonable wrapper see
//! [`CircuitBreakerShared`](super::CircuitBreakerShared).

extern crate std;

use core::time::Duration;
use std::time::Instant;

use super::{circuit_breaker_open_error::CircuitBreakerOpenError, circuit_breaker_state::CircuitBreakerState};

#[cfg(test)]
mod tests;

/// Three-state circuit breaker (Closed / Open / HalfOpen).
///
/// # State transitions
///
/// * **Closed → Open** — when the consecutive failure count reaches `max_failures`.
/// * **Open → HalfOpen** — when `reset_timeout` has elapsed since the circuit opened.
/// * **HalfOpen → Closed** — when a probe call succeeds.
/// * **HalfOpen → Open** — when a probe call fails.
#[derive(Debug)]
pub struct CircuitBreaker {
  max_failures:        u32,
  reset_timeout:       Duration,
  state:               CircuitBreakerState,
  failure_count:       u32,
  opened_at:           Option<Instant>,
  half_open_attempted: bool,
}

impl CircuitBreaker {
  /// Creates a new circuit breaker in the **Closed** state.
  ///
  /// * `max_failures` — number of consecutive failures before the circuit trips.
  /// * `reset_timeout` — how long to wait in the **Open** state before allowing a probe call.
  #[must_use]
  pub const fn new(max_failures: u32, reset_timeout: Duration) -> Self {
    Self {
      max_failures,
      reset_timeout,
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
        let elapsed = opened_at.elapsed();
        if elapsed >= self.reset_timeout {
          // Transition to HalfOpen and permit the probe call.
          self.state = CircuitBreakerState::HalfOpen;
          self.half_open_attempted = true;
          Ok(())
        } else {
          Err(CircuitBreakerOpenError::new(self.reset_timeout - elapsed))
        }
      },
      | CircuitBreakerState::HalfOpen => {
        if self.half_open_attempted {
          // A probe call is already in progress — reject.
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
        // Reset consecutive failure count on success.
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
    self.opened_at = Some(Instant::now());
    self.half_open_attempted = false;
  }

  const fn transition_to_closed(&mut self) {
    self.state = CircuitBreakerState::Closed;
    self.failure_count = 0;
    self.opened_at = None;
    self.half_open_attempted = false;
  }

  fn opened_at_or_now(&self) -> Instant {
    self.opened_at.unwrap_or_else(Instant::now)
  }

  fn remaining_in_open(&self) -> Duration {
    self.opened_at.map_or(Duration::ZERO, |at| self.reset_timeout.saturating_sub(at.elapsed()))
  }
}
