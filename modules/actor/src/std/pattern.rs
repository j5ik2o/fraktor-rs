//! Pekko-inspired helper patterns for the standard toolbox.

use core::{future::Future, time::Duration};

use fraktor_utils_rs::core::timing::delay::DelayProvider;

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
pub use circuit_breaker::CircuitBreaker;
pub use circuit_breaker_call_error::CircuitBreakerCallError;
pub use circuit_breaker_open_error::CircuitBreakerOpenError;
pub use circuit_breaker_shared::CircuitBreakerShared;
pub use circuit_breaker_state::CircuitBreakerState;

#[cfg(test)]
mod tests;

/// Sends a request and arranges timeout completion on the returned ask future.
///
/// # Errors
///
/// Returns an error if the request cannot be delivered.
pub fn ask_with_timeout(
  actor_ref: &crate::core::actor::actor_ref::ActorRef,
  message: crate::core::messaging::AnyMessage,
  timeout: Duration,
) -> Result<crate::core::messaging::AskResponse, crate::core::error::SendError> {
  crate::core::pattern::ask_with_timeout(actor_ref, message, timeout)
}

/// Sends `PoisonPill` and waits until the target actor disappears from the system registry.
///
/// # Errors
///
/// Returns [`crate::core::messaging::AskError::SendFailed`] when the stop message cannot be
/// delivered, or [`crate::core::messaging::AskError::Timeout`] when the actor does not stop
/// before `timeout`.
pub async fn graceful_stop(
  target: &crate::core::actor::actor_ref::ActorRef,
  timeout: Duration,
) -> Result<(), crate::core::messaging::AskError> {
  crate::core::pattern::graceful_stop(target, timeout).await
}

/// Sends the supplied stop message and waits until the target actor disappears from the system
/// registry.
///
/// # Errors
///
/// Returns [`crate::core::messaging::AskError::SendFailed`] when the stop message cannot be
/// delivered, or [`crate::core::messaging::AskError::Timeout`] when the actor does not stop
/// before `timeout`.
pub async fn graceful_stop_with_message(
  target: &crate::core::actor::actor_ref::ActorRef,
  stop_message: crate::core::messaging::AnyMessage,
  timeout: Duration,
) -> Result<(), crate::core::messaging::AskError> {
  crate::core::pattern::graceful_stop_with_message(target, stop_message, timeout).await
}

/// Retries an async operation up to `attempts` times with caller-provided delays.
///
/// # Errors
///
/// Returns the last error produced by `operation` when all attempts are exhausted.
///
/// # Panics
///
/// Panics when `attempts` is zero.
pub async fn retry<T, E, F, Fut, D>(
  attempts: usize,
  delay_provider: &mut impl DelayProvider,
  delay_for: D,
  operation: F,
) -> Result<T, E>
where
  F: FnMut() -> Fut,
  Fut: Future<Output = Result<T, E>>,
  D: FnMut(usize) -> Duration, {
  crate::core::pattern::retry(attempts, delay_provider, delay_for, operation).await
}
