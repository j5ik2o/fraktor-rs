//! Pekko-inspired helper patterns built on top of core actor primitives.

mod ask;
/// Inner circuit breaker state machine.
mod circuit_breaker;
/// Error produced by a circuit-breaker-protected call.
mod circuit_breaker_call_error;
/// Error returned when a circuit breaker rejects a call.
mod circuit_breaker_open_error;
/// Thread-safe shared wrapper for the circuit breaker.
mod circuit_breaker_shared;
/// Circuit breaker state representation.
mod circuit_breaker_state;
/// Clock trait for abstracting time access.
mod clock;
mod graceful_stop;
mod retry;

pub use ask::{ask_with_timeout, complete_with_timeout, install_ask_timeout};
pub use circuit_breaker::CircuitBreaker;
pub use circuit_breaker_call_error::CircuitBreakerCallError;
pub use circuit_breaker_open_error::CircuitBreakerOpenError;
pub use circuit_breaker_shared::CircuitBreakerShared;
pub use circuit_breaker_state::CircuitBreakerState;
pub use clock::Clock;
pub use graceful_stop::{graceful_stop, graceful_stop_with_message};
pub use retry::retry;

#[cfg(test)]
#[path = "pattern_test.rs"]
mod tests;
