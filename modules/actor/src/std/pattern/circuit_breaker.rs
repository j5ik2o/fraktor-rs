//! Standard-library circuit breaker constructor tests.
//!
//! The circuit breaker state machine now lives in `core::pattern::circuit_breaker`.
//! This module provides `std`-specific tests that exercise it through the
//! `StdClock`-parameterised type alias.

#[cfg(test)]
mod tests;
