use core::time::Duration;

use super::StreamBufferConfig;

/// Configuration for [`ActorMaterializerGeneric`](super::ActorMaterializerGeneric).
pub struct ActorMaterializerConfig {
  drive_interval: Duration,
  buffer_config:  StreamBufferConfig,
}

impl ActorMaterializerConfig {
  /// Creates a new configuration with defaults.
  #[must_use]
  pub fn new() -> Self {
    Self { drive_interval: Duration::from_millis(10), buffer_config: StreamBufferConfig::default() }
  }

  /// Returns the configured drive interval.
  #[must_use]
  pub const fn drive_interval(&self) -> Duration {
    self.drive_interval
  }

  /// Returns the configured buffer settings.
  #[must_use]
  pub const fn buffer_config(&self) -> StreamBufferConfig {
    self.buffer_config
  }

  /// Updates the drive interval.
  #[must_use]
  pub const fn with_drive_interval(mut self, drive_interval: Duration) -> Self {
    self.drive_interval = drive_interval;
    self
  }

  /// Updates the buffer configuration.
  #[must_use]
  pub const fn with_buffer_config(mut self, buffer_config: StreamBufferConfig) -> Self {
    self.buffer_config = buffer_config;
    self
  }
}

impl Default for ActorMaterializerConfig {
  fn default() -> Self {
    Self::new()
  }
}
