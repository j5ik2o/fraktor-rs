//! Factory contract for [`CircuitBreakerShared`](super::CircuitBreakerShared).

use super::{CircuitBreaker, CircuitBreakerShared, Clock};

/// Materializes [`CircuitBreakerShared`] instances.
pub trait CircuitBreakerSharedFactory<C>: Send + Sync
where
  C: Clock + 'static, {
  /// Creates a shared circuit-breaker wrapper.
  fn create_circuit_breaker_shared(&self, circuit_breaker: CircuitBreaker<C>) -> CircuitBreakerShared<C>;
}
