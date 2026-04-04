//! Pekko-inspired helper patterns for the standard toolbox.

/// Standard-library circuit breaker implementation.
mod circuit_breaker;
/// Standard-library circuit breaker shared wrapper implementation.
mod circuit_breaker_shared;
/// Standard-library clock backed by `std::time::Instant`.
mod std_clock;

#[cfg(test)]
mod tests;

use core::{future::Future, time::Duration};

use fraktor_actor_rs::core::kernel::{
  actor::{
    actor_ref::ActorRef,
    messaging::{AnyMessage, AskError, AskResponse},
  },
  pattern::{
    CircuitBreaker as CoreCircuitBreaker, CircuitBreakerShared as CoreCircuitBreakerShared,
    ask_with_timeout as core_ask_with_timeout, graceful_stop as core_graceful_stop,
    graceful_stop_with_message as core_graceful_stop_with_message, retry as core_retry,
  },
};
use fraktor_utils_rs::core::timing::delay::DelayProvider;
pub use std_clock::StdClock;

/// Inner circuit breaker state machine using the standard clock.
pub type CircuitBreaker = CoreCircuitBreaker<StdClock>;

/// Thread-safe shared circuit breaker using the standard clock.
pub type CircuitBreakerShared = CoreCircuitBreakerShared<StdClock>;

/// Creates a new [`CircuitBreaker`] in the **Closed** state using the real
/// system clock.
///
/// * `max_failures` — number of consecutive failures before the circuit trips. Must be greater than
///   zero.
/// * `reset_timeout` — how long to wait in the **Open** state before allowing a probe call.
///
/// # Panics
///
/// Panics if `max_failures` is zero.
#[must_use]
pub fn circuit_breaker(max_failures: u32, reset_timeout: Duration) -> CircuitBreaker {
  CircuitBreaker::new_with_clock(max_failures, reset_timeout, StdClock)
}

/// Creates a new [`CircuitBreakerShared`] in the **Closed** state using the
/// real system clock.
///
/// * `max_failures` — consecutive failure threshold before the circuit trips. Must be greater than
///   zero.
/// * `reset_timeout` — delay in the **Open** state before a probe call is allowed.
///
/// # Panics
///
/// Panics if `max_failures` is zero.
#[must_use]
pub fn circuit_breaker_shared(max_failures: u32, reset_timeout: Duration) -> CircuitBreakerShared {
  CircuitBreakerShared::new_with_clock(max_failures, reset_timeout, StdClock)
}

/// Sends a request and arranges timeout completion on the returned ask future.
///
/// The returned future resolves with a timeout error when the reply is not
/// observed before the deadline.
#[must_use]
pub fn ask_with_timeout(actor_ref: &mut ActorRef, message: AnyMessage, timeout: Duration) -> AskResponse {
  core_ask_with_timeout(actor_ref, message, timeout)
}

/// Sends `PoisonPill` and waits until the target actor disappears from the system registry.
///
/// # Errors
///
/// Returns [`fraktor_actor_rs::core::kernel::actor::messaging::AskError::SendFailed`] when the stop
/// message cannot be delivered, or
/// [`fraktor_actor_rs::core::kernel::actor::messaging::AskError::Timeout`] when the actor does not
/// stop before `timeout`.
pub async fn graceful_stop(target: &mut ActorRef, timeout: Duration) -> Result<(), AskError> {
  core_graceful_stop(target, timeout).await
}

/// Sends the supplied stop message and waits until the target actor disappears from the system
/// registry.
///
/// # Errors
///
/// Returns [`fraktor_actor_rs::core::kernel::actor::messaging::AskError::SendFailed`] when the stop
/// message cannot be delivered, or
/// [`fraktor_actor_rs::core::kernel::actor::messaging::AskError::Timeout`] when the actor does not
/// stop before `timeout`.
pub async fn graceful_stop_with_message(
  target: &mut ActorRef,
  stop_message: AnyMessage,
  timeout: Duration,
) -> Result<(), AskError> {
  core_graceful_stop_with_message(target, stop_message, timeout).await
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
  core_retry(attempts, delay_provider, delay_for, operation).await
}
