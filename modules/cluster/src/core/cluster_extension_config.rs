//! Declarative configuration for the cluster extension.

#[cfg(test)]
mod tests;

use alloc::string::String;

use crate::core::{cluster_topology::ClusterTopology, pub_sub::PubSubConfig};

/// Configuration applied when installing the cluster extension.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClusterExtensionConfig {
  advertised_address: String,
  metrics_enabled:    bool,
  static_topology:    Option<ClusterTopology>,
  pubsub_config:      PubSubConfig,
}

impl ClusterExtensionConfig {
  /// Creates a configuration with an empty advertised address and metrics disabled.
  #[must_use]
  pub const fn new() -> Self {
    Self {
      advertised_address: String::new(),
      metrics_enabled:    false,
      static_topology:    None,
      pubsub_config:      PubSubConfig::new(core::time::Duration::from_secs(3), core::time::Duration::from_secs(60)),
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
  pub const fn with_metrics_enabled(mut self, enabled: bool) -> Self {
    self.metrics_enabled = enabled;
    self
  }

  /// Returns the configured advertised address.
  #[must_use]
  #[allow(clippy::missing_const_for_fn)]
  pub fn advertised_address(&self) -> &str {
    &self.advertised_address
  }

  /// Returns whether metrics collection is enabled.
  #[must_use]
  pub const fn metrics_enabled(&self) -> bool {
    self.metrics_enabled
  }

  /// Sets the static topology to be published on startup.
  ///
  /// This is useful for testing or scenarios where topology is predetermined.
  #[must_use]
  pub fn with_static_topology(mut self, topology: ClusterTopology) -> Self {
    self.static_topology = Some(topology);
    self
  }

  /// Sets the pub/sub configuration.
  #[must_use]
  pub const fn with_pubsub_config(mut self, config: PubSubConfig) -> Self {
    self.pubsub_config = config;
    self
  }

  /// Returns the configured static topology.
  #[must_use]
  #[allow(clippy::missing_const_for_fn)]
  pub fn static_topology(&self) -> Option<&ClusterTopology> {
    self.static_topology.as_ref()
  }

  /// Returns the pub/sub configuration.
  #[must_use]
  pub const fn pubsub_config(&self) -> &PubSubConfig {
    &self.pubsub_config
  }
}

impl Default for ClusterExtensionConfig {
  fn default() -> Self {
    Self::new()
  }
}
