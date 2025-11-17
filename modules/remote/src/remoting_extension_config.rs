//! Configuration applied when bootstrapping the remoting extension.

use alloc::string::String;
use core::mem;

use fraktor_actor_rs::core::config::RemotingConfig;

/// Configures remoting bootstrap behaviour.
#[derive(Clone, Debug, PartialEq)]
pub struct RemotingExtensionConfig {
  remoting:   RemotingConfig,
  auto_start: bool,
}

impl RemotingExtensionConfig {
  /// Creates a new configuration from an existing remoting config.
  #[must_use]
  pub const fn new(remoting: RemotingConfig) -> Self {
    Self { remoting, auto_start: true }
  }

  /// Sets whether remoting should automatically start during actor system bootstrap.
  #[must_use]
  pub const fn with_auto_start(mut self, auto_start: bool) -> Self {
    self.auto_start = auto_start;
    self
  }

  /// Returns true when auto-start is enabled.
  #[must_use]
  pub const fn auto_start(&self) -> bool {
    self.auto_start
  }

  /// Returns the underlying remoting configuration.
  #[must_use]
  pub const fn remoting(&self) -> &RemotingConfig {
    &self.remoting
  }

  /// Overrides the canonical host.
  #[must_use]
  pub fn with_canonical_host(mut self, host: impl Into<String>) -> Self {
    let cfg = mem::take(&mut self.remoting);
    self.remoting = cfg.with_canonical_host(host);
    self
  }

  /// Overrides the canonical port.
  #[must_use]
  pub fn with_canonical_port(mut self, port: u16) -> Self {
    let cfg = mem::take(&mut self.remoting);
    self.remoting = cfg.with_canonical_port(port);
    self
  }

  /// Overrides the quarantine duration.
  #[must_use]
  pub fn with_quarantine_duration(mut self, duration: core::time::Duration) -> Self {
    let cfg = mem::take(&mut self.remoting);
    self.remoting = cfg.with_quarantine_duration(duration);
    self
  }
}

impl Default for RemotingExtensionConfig {
  fn default() -> Self {
    Self::new(RemotingConfig::default())
  }
}
