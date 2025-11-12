//! Actor system configuration API.

use alloc::string::{String, ToString};

use super::RemotingConfig;

#[cfg(test)]
mod tests;

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
