//! Remoting configuration.

use alloc::string::{String, ToString};
use core::time::Duration;

const MIN_QUARANTINE_DURATION: Duration = Duration::from_secs(1);

/// Configuration for remoting capabilities.
#[derive(Clone, Debug, PartialEq)]
pub struct RemotingConfig {
  canonical_host:      String,
  canonical_port:      Option<u16>,
  quarantine_duration: Duration,
}

impl RemotingConfig {
  /// Sets the canonical hostname for this actor system.
  #[must_use]
  pub fn with_canonical_host(mut self, host: impl Into<String>) -> Self {
    self.canonical_host = host.into();
    self
  }

  /// Sets the canonical port for this actor system.
  #[must_use]
  pub const fn with_canonical_port(mut self, port: u16) -> Self {
    self.canonical_port = Some(port);
    self
  }

  /// Sets the quarantine duration for remote authorities.
  ///
  /// # Panics
  ///
  /// Panics if `duration` is shorter than one second because such a value would violate the
  /// minimum quarantine policy described in the Pekko-compatible actor path design.
  #[must_use]
  pub fn with_quarantine_duration(mut self, duration: Duration) -> Self {
    assert!(duration >= MIN_QUARANTINE_DURATION, "quarantine duration must be >= 1 second");
    self.quarantine_duration = duration;
    self
  }

  /// Returns the canonical hostname.
  #[must_use]
  pub fn canonical_host(&self) -> &str {
    &self.canonical_host
  }

  /// Returns the canonical port.
  #[must_use]
  pub const fn canonical_port(&self) -> Option<u16> {
    self.canonical_port
  }

  /// Returns the quarantine duration.
  #[must_use]
  pub const fn quarantine_duration(&self) -> Duration {
    self.quarantine_duration
  }
}

impl Default for RemotingConfig {
  fn default() -> Self {
    Self {
      canonical_host:      "localhost".to_string(),
      canonical_port:      None,
      quarantine_duration: Duration::from_secs(5 * 24 * 3600), // 5 days
    }
  }
}
