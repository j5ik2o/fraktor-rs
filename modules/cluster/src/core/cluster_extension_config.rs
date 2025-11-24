//! Declarative configuration for the cluster extension.

#[cfg(test)]
mod tests;

use alloc::string::String;

/// Configuration applied when installing the cluster extension.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClusterExtensionConfig {
  advertised_address: String,
  metrics_enabled:    bool,
}

impl ClusterExtensionConfig {
  /// Creates a configuration with an empty advertised address and metrics disabled.
  #[must_use]
  pub fn new() -> Self {
    Self {
      advertised_address: String::new(),
      metrics_enabled:    false,
    }
  }

  /// Overrides the advertised address used in cluster events.
  #[must_use]
  pub fn with_advertised_address(mut self, address: impl Into<String>) -> Self {
    self.advertised_address = address.into();
    self
  }

  /// Enables or disables cluster metrics.
  #[must_use]
  pub fn with_metrics_enabled(mut self, enabled: bool) -> Self {
    self.metrics_enabled = enabled;
    self
  }

  /// Returns the configured advertised address.
  #[must_use]
  pub fn advertised_address(&self) -> &str {
    &self.advertised_address
  }

  /// Returns whether metrics collection is enabled.
  #[must_use]
  pub const fn metrics_enabled(&self) -> bool {
    self.metrics_enabled
  }
}

impl Default for ClusterExtensionConfig {
  fn default() -> Self {
    Self::new()
  }
}
