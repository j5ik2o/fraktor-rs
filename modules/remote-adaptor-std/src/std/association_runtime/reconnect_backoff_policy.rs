//! Outbound reconnect backoff settings.

use core::time::Duration;

/// Backoff, timeout, and restart budget used by the outbound reconnect loop.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReconnectBackoffPolicy {
  backoff:      Duration,
  timeout:      Duration,
  max_restarts: u32,
}

impl ReconnectBackoffPolicy {
  /// Creates a new reconnect policy from already-resolved configuration values.
  #[must_use]
  pub const fn new(backoff: Duration, timeout: Duration, max_restarts: u32) -> Self {
    Self { backoff, timeout, max_restarts }
  }

  /// Returns the delay before attempting a reconnect.
  #[must_use]
  pub const fn backoff(&self) -> Duration {
    self.backoff
  }

  /// Returns the maximum duration allowed for a reconnect attempt.
  #[must_use]
  pub const fn timeout(&self) -> Duration {
    self.timeout
  }

  /// Returns the maximum number of reconnect attempts after send failure.
  #[must_use]
  pub const fn max_restarts(&self) -> u32 {
    self.max_restarts
  }
}
