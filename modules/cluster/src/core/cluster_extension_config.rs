//! Declarative configuration for the cluster extension.

#[cfg(test)]
mod tests;

use alloc::{
  string::{String, ToString},
  vec::Vec,
};

use crate::core::{
  ConfigValidation, JoinConfigCompatChecker, cluster_topology::ClusterTopology, pub_sub::PubSubConfig,
};

/// Configuration applied when installing the cluster extension.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClusterExtensionConfig {
  advertised_address: String,
  metrics_enabled:    bool,
  static_topology:    Option<ClusterTopology>,
  pubsub_config:      PubSubConfig,
  app_version:        String,
  roles:              Vec<String>,
}

impl ClusterExtensionConfig {
  /// Creates a configuration with an empty advertised address and metrics disabled.
  ///
  /// `app_version` defaults to the `fraktor-cluster-rs` crate version via
  /// `env!("CARGO_PKG_VERSION")`. Use [`with_app_version`](Self::with_app_version)
  /// to override it with the embedding application's version.
  #[must_use]
  pub fn new() -> Self {
    Self {
      advertised_address: String::new(),
      metrics_enabled:    false,
      static_topology:    None,
      pubsub_config:      PubSubConfig::new(core::time::Duration::from_secs(3), core::time::Duration::from_secs(60)),
      app_version:        String::from(env!("CARGO_PKG_VERSION")),
      roles:              Vec::new(),
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

  /// Sets cluster roles advertised by this node.
  #[must_use]
  pub fn with_roles(mut self, roles: Vec<String>) -> Self {
    self.roles = normalize_roles(roles);
    self
  }

  /// Sets application version advertised by this node.
  #[must_use]
  pub fn with_app_version(mut self, app_version: impl Into<String>) -> Self {
    self.app_version = app_version.into();
    self
  }

  /// Returns the configured static topology.
  #[must_use]
  pub const fn static_topology(&self) -> Option<&ClusterTopology> {
    self.static_topology.as_ref()
  }

  /// Returns the pub/sub configuration.
  #[must_use]
  pub const fn pubsub_config(&self) -> &PubSubConfig {
    &self.pubsub_config
  }

  /// Returns advertised application version.
  #[must_use]
  pub fn app_version(&self) -> &str {
    &self.app_version
  }

  /// Returns configured cluster roles.
  #[must_use]
  pub fn roles(&self) -> &[String] {
    &self.roles
  }
}

impl Default for ClusterExtensionConfig {
  fn default() -> Self {
    Self::new()
  }
}

impl JoinConfigCompatChecker for ClusterExtensionConfig {
  fn check_join_compatibility(&self, joining: &ClusterExtensionConfig) -> ConfigValidation {
    // TODO(#248): gossip_config、app_version、roles 等の互換性チェックを追加する
    if self.pubsub_config != joining.pubsub_config {
      return ConfigValidation::Incompatible { reason: "pubsub configuration mismatch".to_string() };
    }
    ConfigValidation::Compatible
  }
}

fn normalize_roles(mut roles: Vec<String>) -> Vec<String> {
  roles.sort();
  roles.dedup();
  roles
}
