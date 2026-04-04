//! Standard-library circuit breaker shared wrapper tests.
//!
//! The shared circuit breaker now lives in `core::pattern::circuit_breaker_shared`.
//! This module provides `std`-specific tests that exercise it through the
//! `StdClock`-parameterised type alias.

#[cfg(test)]
mod tests;
