use core::time::Duration;

use fraktor_actor_rs::core::kernel::pattern::{
  CircuitBreaker as CoreCircuitBreaker, CircuitBreakerShared as CoreCircuitBreakerShared,
};

use crate::std::time::StdClock;

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
