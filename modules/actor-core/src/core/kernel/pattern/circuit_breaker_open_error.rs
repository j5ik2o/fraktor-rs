//! Error returned when a circuit breaker rejects a call.

use alloc::fmt;
use core::time::Duration;

/// Error indicating that the circuit breaker is open and rejecting calls.
#[derive(Debug, Clone)]
pub struct CircuitBreakerOpenError {
  remaining: Duration,
}

impl CircuitBreakerOpenError {
  /// Creates a new error with the remaining duration until the circuit may reset.
  #[must_use]
  pub const fn new(remaining: Duration) -> Self {
    Self { remaining }
  }

  /// Returns the remaining duration until the circuit breaker may attempt a reset.
  #[must_use]
  pub const fn remaining(&self) -> Duration {
    self.remaining
  }
}

impl fmt::Display for CircuitBreakerOpenError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "circuit breaker is open; remaining {:?}", self.remaining)
  }
}
