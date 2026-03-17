//! Phase configuration for coordinated shutdown.

extern crate std;

use alloc::{string::String, vec::Vec};
use core::time::Duration;

/// Configuration for a single coordinated shutdown phase.
#[derive(Debug, Clone)]
pub struct CoordinatedShutdownPhase {
  depends_on: Vec<String>,
  timeout:    Duration,
  recover:    bool,
  enabled:    bool,
}

impl CoordinatedShutdownPhase {
  /// Creates a new phase with the given dependencies and timeout.
  #[must_use]
  pub const fn new(depends_on: Vec<String>, timeout: Duration) -> Self {
    Self { depends_on, timeout, recover: true, enabled: true }
  }

  /// Returns the list of phase names that this phase depends on.
  #[must_use]
  pub fn depends_on(&self) -> &[String] {
    &self.depends_on
  }

  /// Returns the timeout for this phase.
  #[must_use]
  pub const fn timeout(&self) -> Duration {
    self.timeout
  }

  /// Returns whether this phase should recover from task failures.
  #[must_use]
  pub const fn recover(&self) -> bool {
    self.recover
  }

  /// Returns whether this phase is enabled.
  #[must_use]
  pub const fn enabled(&self) -> bool {
    self.enabled
  }

  /// Sets the recover flag.
  #[must_use]
  pub const fn with_recover(mut self, recover: bool) -> Self {
    self.recover = recover;
    self
  }

  /// Sets the enabled flag.
  #[must_use]
  pub const fn with_enabled(mut self, enabled: bool) -> Self {
    self.enabled = enabled;
    self
  }
}
