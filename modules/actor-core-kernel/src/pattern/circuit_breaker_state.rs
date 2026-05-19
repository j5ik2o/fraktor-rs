//! Circuit breaker state representation.

/// Represents the current state of a circuit breaker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitBreakerState {
  /// The circuit is closed (normal operation). Calls are allowed through.
  Closed,
  /// The circuit is open (tripped). Calls are rejected immediately.
  Open,
  /// The circuit is half-open (testing recovery). A single probe call is permitted.
  HalfOpen,
}
