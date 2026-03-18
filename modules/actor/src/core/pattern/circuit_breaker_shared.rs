//! Thread-safe shared wrapper for [`CircuitBreaker`](super::CircuitBreaker).

use core::{future::Future, time::Duration};

use fraktor_utils_rs::core::sync::{ArcShared, RuntimeMutex, SharedAccess};

use super::{
  circuit_breaker::CircuitBreaker, circuit_breaker_call_error::CircuitBreakerCallError,
  circuit_breaker_state::CircuitBreakerState, clock::Clock,
};

#[cfg(test)]
mod tests;

/// Thread-safe, clonable circuit breaker.
///
/// Wraps [`CircuitBreaker`] in `ArcShared<RuntimeMutex<..>>` and provides an
/// async [`call`](Self::call) method that checks permission, executes the
/// operation, and records the outcome automatically.
pub struct CircuitBreakerShared<C: Clock> {
  inner: ArcShared<RuntimeMutex<CircuitBreaker<C>>>,
}

impl<C: Clock> Clone for CircuitBreakerShared<C> {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}

impl<C: Clock> CircuitBreakerShared<C> {
  /// Creates a new shared circuit breaker in the **Closed** state.
  ///
  /// * `max_failures` — consecutive failure threshold before the circuit trips.
  /// * `reset_timeout` — delay in the **Open** state before a probe call is allowed.
  /// * `clock` — a [`Clock`] implementation for obtaining the current time.
  #[must_use]
  pub fn new_with_clock(max_failures: u32, reset_timeout: Duration, clock: C) -> Self {
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
    // 許可判定時の状態を記録する。await 後に状態が変わっている可能性があるため、
    // 結果反映時にこの情報を使って HalfOpen probe の判定を正しく行う。
    let state_at_permit = self.with_write(|cb| {
      cb.is_call_permitted()?;
      Ok::<_, super::circuit_breaker_open_error::CircuitBreakerOpenError>(cb.state())
    }).map_err(CircuitBreakerCallError::Open)?;

    let was_half_open = state_at_permit == CircuitBreakerState::HalfOpen;
    let mut guard = CallGuard { cb: self.clone(), was_half_open, disarmed: false };

    let result = operation().await;

    guard.disarmed = true;

    match result {
      | Ok(value) => {
        self.with_write(|cb| cb.record_success_for(was_half_open));
        Ok(value)
      },
      | Err(e) => {
        self.with_write(|cb| cb.record_failure_for(was_half_open));
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
struct CallGuard<C: Clock> {
  cb:            CircuitBreakerShared<C>,
  was_half_open: bool,
  disarmed:      bool,
}

impl<C: Clock> Drop for CallGuard<C> {
  fn drop(&mut self) {
    if !self.disarmed {
      self.cb.with_write(|cb| cb.record_failure_for(self.was_half_open));
    }
  }
}

impl<C: Clock> SharedAccess<CircuitBreaker<C>> for CircuitBreakerShared<C> {
  #[inline]
  fn with_read<R>(&self, f: impl FnOnce(&CircuitBreaker<C>) -> R) -> R {
    let guard = self.inner.lock();
    f(&guard)
  }

  #[inline]
  fn with_write<R>(&self, f: impl FnOnce(&mut CircuitBreaker<C>) -> R) -> R {
    let mut guard = self.inner.lock();
    f(&mut guard)
  }
}
