//! Bootstrap-time actor-system setup facade.

#[cfg(test)]
mod tests;

use alloc::string::String;
use core::time::Duration;

use crate::{
  actor::{actor_path::GuardianKind as PathGuardianKind, setup::ActorSystemConfig},
  system::remote::RemotingConfig,
};

/// Pekko-compatible bootstrap setup facade backed by [`ActorSystemConfig`].
pub struct BootstrapSetup {
  config: ActorSystemConfig,
}

impl BootstrapSetup {
  /// Creates a new bootstrap setup from the provided actor-system config.
  #[must_use]
  pub const fn new(config: ActorSystemConfig) -> Self {
    Self { config }
  }

  /// Sets the actor system name.
  #[must_use]
  pub fn with_system_name(self, name: impl Into<String>) -> Self {
    Self::new(self.config.with_system_name(name))
  }

  /// Sets the default guardian segment.
  #[must_use]
  pub fn with_default_guardian(self, guardian: PathGuardianKind) -> Self {
    Self::new(self.config.with_default_guardian(guardian))
  }

  /// Enables remoting with the provided configuration.
  #[must_use]
  pub fn with_remoting_config(self, config: RemotingConfig) -> Self {
    Self::new(self.config.with_remoting_config(config))
  }

  /// Sets the actor-system start time.
  #[must_use]
  pub fn with_start_time(self, start_time: Duration) -> Self {
    Self::new(self.config.with_start_time(start_time))
  }

  /// Returns the underlying actor-system config.
  #[must_use]
  pub const fn as_actor_system_config(&self) -> &ActorSystemConfig {
    &self.config
  }

  /// Consumes the setup and returns the underlying actor-system config.
  #[must_use]
  pub fn into_actor_system_config(self) -> ActorSystemConfig {
    self.config
  }
}

impl Default for BootstrapSetup {
  fn default() -> Self {
    Self::new(ActorSystemConfig::default())
  }
}

impl From<BootstrapSetup> for ActorSystemConfig {
  fn from(value: BootstrapSetup) -> Self {
    value.into_actor_system_config()
  }
}
