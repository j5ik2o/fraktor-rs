//! Error produced by [`CircuitBreakerShared::call`](super::CircuitBreakerShared::call).

use alloc::fmt;

use super::circuit_breaker_open_error::CircuitBreakerOpenError;

/// Error returned from a circuit-breaker-protected call.
pub enum CircuitBreakerCallError<E> {
  /// The circuit breaker is open and the call was not attempted.
  Open(CircuitBreakerOpenError),
  /// The underlying operation returned an error.
  Failed(E),
}

impl<E: fmt::Debug> fmt::Debug for CircuitBreakerCallError<E> {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      | Self::Open(err) => f.debug_tuple("Open").field(err).finish(),
      | Self::Failed(err) => f.debug_tuple("Failed").field(err).finish(),
    }
  }
}

impl<E: fmt::Display> fmt::Display for CircuitBreakerCallError<E> {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      | Self::Open(err) => write!(f, "{err}"),
      | Self::Failed(err) => write!(f, "call failed: {err}"),
    }
  }
}
