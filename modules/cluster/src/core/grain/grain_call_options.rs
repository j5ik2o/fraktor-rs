//! Call options for grain requests.

#[cfg(test)]
mod tests;

use core::time::Duration;

use super::GrainRetryPolicy;

/// Call options for grain requests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GrainCallOptions {
  /// Optional timeout applied to each request.
  pub timeout: Option<Duration>,
  /// Retry policy for lookup failures.
  pub retry:   GrainRetryPolicy,
}

impl GrainCallOptions {
  /// Creates a new set of call options.
  #[must_use]
  pub const fn new(timeout: Option<Duration>, retry: GrainRetryPolicy) -> Self {
    Self { timeout, retry }
  }
}

impl Default for GrainCallOptions {
  fn default() -> Self {
    Self { timeout: None, retry: GrainRetryPolicy::NoRetry }
  }
}
