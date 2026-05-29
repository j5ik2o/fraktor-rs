//! Declarative configuration for the cluster extension.

#[cfg(test)]
#[path = "cluster_extension_config_test.rs"]
mod tests;

use alloc::{
  string::{String, ToString},
  vec::Vec,
};
use core::time::Duration;

use crate::{
  ClusterTopology, ConfigValidation, JoinConfigCompatChecker, downing_provider::DowningProviderCompatibility,
  pub_sub::PubSubConfig,
};

const PUBSUB_CONFIGURATION_MISMATCH_REASON: &str = "pubsub configuration mismatch";
const DOWNING_PROVIDER_KEY_MISMATCH_REASON: &str = "downing provider compatibility key mismatch";
const SBR_SETTINGS_MISMATCH_REASON: &str = "split brain resolver settings mismatch";
const SPLIT_BRAIN_RESOLVER_PROVIDER_KEY: &str = "split-brain-resolver";

/// Configuration applied when installing the cluster extension.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClusterExtensionConfig {
  advertised_address: String,
  metrics_enabled:    bool,
  static_topology:    Option<ClusterTopology>,
  pubsub_config:      PubSubConfig,
  app_version:        String,
  roles:              Vec<String>,
  downing_provider:   DowningProviderCompatibility,
}

impl ClusterExtensionConfig {
  /// Creates a configuration with an empty advertised address and metrics disabled.
  ///
  /// `app_version` defaults to the `fraktor-cluster-core-kernel-rs` crate version via
  /// `env!("CARGO_PKG_VERSION")`. Use [`with_app_version`](Self::with_app_version)
  /// to override it with the embedding application's version.
  #[must_use]
  pub fn new() -> Self {
    Self {
      advertised_address: String::new(),
      metrics_enabled:    false,
      static_topology:    None,
      pubsub_config:      PubSubConfig::new(Duration::from_secs(3), Duration::from_secs(60)),
      app_version:        String::from(env!("CARGO_PKG_VERSION")),
      roles:              Vec::new(),
      downing_provider:   DowningProviderCompatibility::noop(),
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

  /// Sets the downing provider compatibility identity.
  #[must_use]
  pub fn with_downing_provider_compatibility(mut self, compatibility: DowningProviderCompatibility) -> Self {
    self.downing_provider = compatibility;
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

  /// Returns the downing provider compatibility identity.
  #[must_use]
  pub const fn downing_provider_compatibility(&self) -> &DowningProviderCompatibility {
    &self.downing_provider
  }
}

impl Default for ClusterExtensionConfig {
  fn default() -> Self {
    Self::new()
  }
}

impl JoinConfigCompatChecker for ClusterExtensionConfig {
  fn check_join_compatibility(&self, joining: &ClusterExtensionConfig) -> ConfigValidation {
    if self.pubsub_config != joining.pubsub_config {
      return ConfigValidation::Incompatible { reason: PUBSUB_CONFIGURATION_MISMATCH_REASON.to_string() };
    }
    if self.downing_provider.provider_key() != joining.downing_provider.provider_key() {
      return ConfigValidation::Incompatible { reason: DOWNING_PROVIDER_KEY_MISMATCH_REASON.to_string() };
    }
    if self.downing_provider.provider_key() == SPLIT_BRAIN_RESOLVER_PROVIDER_KEY
      && self.downing_provider.split_brain_resolver_settings()
        != joining.downing_provider.split_brain_resolver_settings()
    {
      return ConfigValidation::Incompatible { reason: SBR_SETTINGS_MISMATCH_REASON.to_string() };
    }
    ConfigValidation::Compatible
  }
}

fn normalize_roles(mut roles: Vec<String>) -> Vec<String> {
  roles.sort();
  roles.dedup();
  roles
}
