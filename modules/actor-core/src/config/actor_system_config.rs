//! Actor system configuration API.

use alloc::string::{String, ToString};
use core::time::Duration;

#[cfg(test)]
mod tests;

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
  #[must_use]
  pub const fn with_quarantine_duration(mut self, duration: Duration) -> Self {
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

/// Configuration for the actor system.
#[derive(Clone, Debug)]
pub struct ActorSystemConfig {
  system_name: String,
  remoting:    Option<RemotingConfig>,
}

impl ActorSystemConfig {
  /// Sets the actor system name.
  #[must_use]
  pub fn with_system_name(mut self, name: impl Into<String>) -> Self {
    self.system_name = name.into();
    self
  }

  /// Enables remoting with the given configuration.
  #[must_use]
  pub fn with_remoting(mut self, config: RemotingConfig) -> Self {
    self.remoting = Some(config);
    self
  }

  /// Returns the system name.
  #[must_use]
  pub fn system_name(&self) -> &str {
    &self.system_name
  }

  /// Returns the remoting configuration if enabled.
  #[must_use]
  pub const fn remoting(&self) -> Option<&RemotingConfig> {
    self.remoting.as_ref()
  }
}

impl Default for ActorSystemConfig {
  fn default() -> Self {
    Self { system_name: "default-system".to_string(), remoting: None }
  }
}
