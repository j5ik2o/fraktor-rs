//! Error returned when a circuit breaker rejects a call.

use core::{
  fmt::{Display, Formatter, Result as FmtResult},
  time::Duration,
};

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

impl Display for CircuitBreakerOpenError {
  fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
    write!(f, "circuit breaker is open; remaining {:?}", self.remaining)
  }
}
