//! Minimal retry helper.

use core::{future::Future, time::Duration};

use fraktor_utils_rs::core::timing::delay::{DelayFuture, DelayProvider};

/// Retries an async operation up to `attempts` times with caller-provided delays.
///
/// The delay closure receives the 1-based retry index. For example, the first retry
/// after an initial failure is invoked with `1`.
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
  mut delay_for: D,
  mut operation: F,
) -> Result<T, E>
where
  F: FnMut() -> Fut,
  Fut: Future<Output = Result<T, E>>,
  D: FnMut(usize) -> Duration, {
  assert!(attempts > 0, "retry attempts must be greater than zero");

  let mut current_attempt = 1;
  loop {
    match operation().await {
      | Ok(value) => return Ok(value),
      | Err(error) => {
        if current_attempt >= attempts {
          return Err(error);
        }
        let delay: DelayFuture = delay_provider.delay(delay_for(current_attempt));
        delay.await;
        current_attempt += 1;
      },
    }
  }
}
