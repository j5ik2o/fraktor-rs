//! Thread-safe shared wrapper for [`CircuitBreaker`](super::CircuitBreaker).

extern crate std;

use core::{future::Future, time::Duration};
use std::time::Instant;

use fraktor_utils_rs::core::sync::{ArcShared, RuntimeMutex, SharedAccess};

use super::{
  circuit_breaker::CircuitBreaker, circuit_breaker_call_error::CircuitBreakerCallError,
  circuit_breaker_state::CircuitBreakerState,
};

#[cfg(test)]
mod tests;

/// Thread-safe, clonable circuit breaker.
///
/// Wraps [`CircuitBreaker`] in `ArcShared<RuntimeMutex<..>>` and provides an
/// async [`call`](Self::call) method that checks permission, executes the
/// operation, and records the outcome automatically.
#[derive(Clone)]
pub struct CircuitBreakerShared {
  inner: ArcShared<RuntimeMutex<CircuitBreaker>>,
}

impl CircuitBreakerShared {
  /// Creates a new shared circuit breaker in the **Closed** state.
  ///
  /// * `max_failures` — consecutive failure threshold before the circuit trips.
  /// * `reset_timeout` — delay in the **Open** state before a probe call is allowed.
  #[must_use]
  pub fn new(max_failures: u32, reset_timeout: Duration) -> Self {
    Self { inner: ArcShared::new(RuntimeMutex::new(CircuitBreaker::new(max_failures, reset_timeout))) }
  }

  /// Creates a new shared circuit breaker with a custom clock function.
  ///
  /// See [`CircuitBreaker::new_with_clock`] for details on the `clock` parameter.
  #[must_use]
  pub(crate) fn new_with_clock(
    max_failures: u32,
    reset_timeout: Duration,
    clock: impl Fn() -> Instant + Send + Sync + 'static,
  ) -> Self {
    Self {
      inner: ArcShared::new(RuntimeMutex::new(CircuitBreaker::new_with_clock(max_failures, reset_timeout, clock))),
    }
  }

  /// Executes `operation` through the circuit breaker.
  ///
  /// 1. Checks whether a call is currently permitted.
  /// 2. If permitted, runs `operation` **without** holding the lock.
  /// 3. Records the outcome (success / failure) and updates the state.
  ///
  /// Timeout enforcement is **not** built in — wrap `operation` with your own
  /// timeout mechanism (e.g. `tokio::time::timeout`) and return an error on
  /// expiry.
  ///
  /// # Errors
  ///
  /// Returns [`CircuitBreakerCallError::Open`] when the circuit is open, or
  /// [`CircuitBreakerCallError::Failed`] when `operation` returns `Err`.
  pub async fn call<T, E, F, Fut>(&self, operation: F) -> Result<T, CircuitBreakerCallError<E>>
  where
    F: FnOnce() -> Fut,
    Fut: Future<Output = Result<T, E>>, {
    self.with_write(|cb| cb.is_call_permitted()).map_err(CircuitBreakerCallError::Open)?;

    let mut guard = CallGuard { cb: self.clone(), disarmed: false };

    let result = operation().await;

    guard.disarmed = true;

    match result {
      | Ok(value) => {
        self.with_write(|cb| cb.record_success());
        Ok(value)
      },
      | Err(e) => {
        self.with_write(|cb| cb.record_failure());
        Err(CircuitBreakerCallError::Failed(e))
      },
    }
  }

  /// Returns the current state of the circuit breaker.
  #[must_use]
  pub fn state(&self) -> CircuitBreakerState {
    self.with_read(|cb| cb.state())
  }

  /// Returns the current consecutive failure count.
  #[must_use]
  pub fn failure_count(&self) -> u32 {
    self.with_read(|cb| cb.failure_count())
  }
}

/// RAII guard that records a failure when dropped without being disarmed.
///
/// Used to ensure cancel safety: if the operation future is dropped mid-flight
/// (e.g. via `tokio::time::timeout`), the circuit breaker transitions out of
/// HalfOpen instead of getting stuck.
struct CallGuard {
  cb:       CircuitBreakerShared,
  disarmed: bool,
}

impl Drop for CallGuard {
  fn drop(&mut self) {
    if !self.disarmed {
      self.cb.with_write(|cb| cb.record_failure());
    }
  }
}

impl SharedAccess<CircuitBreaker> for CircuitBreakerShared {
  #[inline]
  fn with_read<R>(&self, f: impl FnOnce(&CircuitBreaker) -> R) -> R {
    let guard = self.inner.lock();
    f(&guard)
  }

  #[inline]
  fn with_write<R>(&self, f: impl FnOnce(&mut CircuitBreaker) -> R) -> R {
    let mut guard = self.inner.lock();
    f(&mut guard)
  }
}
