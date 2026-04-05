//! Pekko-inspired helper patterns for the standard toolbox.

mod circuit_breaker_bindings;

#[cfg(test)]
mod tests;

pub use circuit_breaker_bindings::{CircuitBreaker, CircuitBreakerShared, circuit_breaker, circuit_breaker_shared};
