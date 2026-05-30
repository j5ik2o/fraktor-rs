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
const PUBSUB_SUBSCRIBER_TIMEOUT_KEY: &str = "fraktor.cluster.pubsub.subscriber-timeout";
const PUBSUB_SUSPENDED_TTL_KEY: &str = "fraktor.cluster.pubsub.suspended-ttl";
const DOWNING_PROVIDER_KEY: &str = "fraktor.cluster.downing-provider.provider-key";
const SBR_STABLE_AFTER_KEY: &str = "fraktor.cluster.downing-provider.split-brain-resolver.stable-after";
const SBR_ACTIVE_STRATEGY_KEY: &str = "fraktor.cluster.downing-provider.split-brain-resolver.active-strategy";
const SBR_DOWN_ALL_WHEN_UNSTABLE_KEY: &str =
  "fraktor.cluster.downing-provider.split-brain-resolver.down-all-when-unstable";
const JOIN_COMPATIBILITY_KEYS: &[&str] = &[
  PUBSUB_SUBSCRIBER_TIMEOUT_KEY,
  PUBSUB_SUSPENDED_TTL_KEY,
  DOWNING_PROVIDER_KEY,
  SBR_STABLE_AFTER_KEY,
  SBR_ACTIVE_STRATEGY_KEY,
  SBR_DOWN_ALL_WHEN_UNSTABLE_KEY,
];
const SENSITIVE_JOIN_COMPATIBILITY_KEYS: &[&str] = &[];

struct JoinCompatibilityCheck {
  reason:        &'static str,
  is_compatible: fn(&ClusterExtensionConfig, &ClusterExtensionConfig) -> bool,
}

impl JoinCompatibilityCheck {
  const fn new(
    reason: &'static str,
    is_compatible: fn(&ClusterExtensionConfig, &ClusterExtensionConfig) -> bool,
  ) -> Self {
    Self { reason, is_compatible }
  }

  fn check(&self, local: &ClusterExtensionConfig, joining: &ClusterExtensionConfig) -> ConfigValidation {
    if (self.is_compatible)(local, joining) {
      ConfigValidation::Compatible
    } else {
      ConfigValidation::Incompatible { reason: self.reason.to_string() }
    }
  }
}

const JOIN_COMPATIBILITY_CHECKS: &[JoinCompatibilityCheck] = &[
  JoinCompatibilityCheck::new(PUBSUB_CONFIGURATION_MISMATCH_REASON, pubsub_configs_are_compatible),
  JoinCompatibilityCheck::new(DOWNING_PROVIDER_KEY_MISMATCH_REASON, downing_provider_keys_are_compatible),
  JoinCompatibilityCheck::new(SBR_SETTINGS_MISMATCH_REASON, split_brain_resolver_settings_are_compatible),
];

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

  /// Returns non-sensitive configuration keys that must match before accepting a join.
  #[must_use]
  pub const fn required_join_compatibility_keys() -> &'static [&'static str] {
    JOIN_COMPATIBILITY_KEYS
  }

  /// Returns configuration keys excluded from advertised join compatibility metadata.
  #[must_use]
  pub const fn sensitive_join_compatibility_keys() -> &'static [&'static str] {
    SENSITIVE_JOIN_COMPATIBILITY_KEYS
  }

  /// Returns whether the key participates in join compatibility checks.
  #[must_use]
  pub fn is_required_join_compatibility_key(key: &str) -> bool {
    JOIN_COMPATIBILITY_KEYS.contains(&key)
  }

  /// Returns whether the key must not be advertised in join compatibility metadata.
  #[must_use]
  pub fn is_sensitive_join_compatibility_key(key: &str) -> bool {
    SENSITIVE_JOIN_COMPATIBILITY_KEYS.contains(&key)
  }
}

impl Default for ClusterExtensionConfig {
  fn default() -> Self {
    Self::new()
  }
}

impl JoinConfigCompatChecker for ClusterExtensionConfig {
  fn check_join_compatibility(&self, joining: &ClusterExtensionConfig) -> ConfigValidation {
    for check in JOIN_COMPATIBILITY_CHECKS {
      let validation = check.check(self, joining);
      if let ConfigValidation::Incompatible { .. } = validation {
        return validation;
      }
    }
    ConfigValidation::Compatible
  }
}

fn pubsub_configs_are_compatible(local: &ClusterExtensionConfig, joining: &ClusterExtensionConfig) -> bool {
  local.pubsub_config == joining.pubsub_config
}

fn downing_provider_keys_are_compatible(local: &ClusterExtensionConfig, joining: &ClusterExtensionConfig) -> bool {
  local.downing_provider.provider_key() == joining.downing_provider.provider_key()
}

fn split_brain_resolver_settings_are_compatible(
  local: &ClusterExtensionConfig,
  joining: &ClusterExtensionConfig,
) -> bool {
  local.downing_provider.provider_key() != SPLIT_BRAIN_RESOLVER_PROVIDER_KEY
    || local.downing_provider.split_brain_resolver_settings()
      == joining.downing_provider.split_brain_resolver_settings()
}

fn normalize_roles(mut roles: Vec<String>) -> Vec<String> {
  roles.sort();
  roles.dedup();
  roles
}
