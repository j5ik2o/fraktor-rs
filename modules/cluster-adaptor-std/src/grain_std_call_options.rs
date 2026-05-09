//! Std helpers for grain call options.

use core::time::Duration;

use fraktor_cluster_core_rs::grain::{GrainCallOptions, GrainRetryPolicy};

#[cfg(test)]
mod tests;

/// Returns the std default call options (mirrors core defaults).
#[must_use]
pub fn default_grain_call_options() -> GrainCallOptions {
  GrainCallOptions::default()
}

/// Returns call options with a timeout and no retries.
#[must_use]
pub fn call_options_with_timeout(timeout: Duration) -> GrainCallOptions {
  GrainCallOptions::new(Some(timeout), GrainRetryPolicy::NoRetry)
}

/// Returns call options with timeout and retry policy.
#[must_use]
pub fn call_options_with_retry(timeout: Duration, retry: GrainRetryPolicy) -> GrainCallOptions {
  GrainCallOptions::new(Some(timeout), retry)
}
