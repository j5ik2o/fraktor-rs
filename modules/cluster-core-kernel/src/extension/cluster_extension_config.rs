//! Declarative configuration for the cluster extension.

#[cfg(test)]
#[path = "cluster_extension_config_test.rs"]
mod tests;

use alloc::{string::String, vec::Vec};
use core::time::Duration;

use crate::{
  ClusterExtensionConfigError, ClusterShardingStateStoreMode, ClusterTopology, ConfigValidation,
  JoinConfigCompatChecker,
  downing_provider::DowningProviderCompatibility,
  failure_detector::FailureDetectorConfig,
  pub_sub::PubSubConfig,
  singleton::{ClusterSingletonConfigError, ClusterSingletonManagerConfig, ClusterSingletonProxyConfig},
  topology::{ClusterCompatibilityKey, ClusterCompatibilityKeyCatalog},
};

const PUBSUB_CONFIGURATION_MISMATCH_REASON: &str = "pubsub configuration mismatch";
const DOWNING_PROVIDER_KEY_MISMATCH_REASON: &str = "downing provider compatibility key mismatch";
const SBR_CONFIG_MISMATCH_REASON: &str = "split brain resolver config mismatch";
const SPLIT_BRAIN_RESOLVER_PROVIDER_KEY: &str = "split-brain-resolver";
const PUBSUB_SUBSCRIBER_TIMEOUT_KEY: &str = "fraktor.cluster.pubsub.subscriber-timeout";
const PUBSUB_SUSPENDED_TTL_KEY: &str = "fraktor.cluster.pubsub.suspended-ttl";
const DOWNING_PROVIDER_KEY: &str = "fraktor.cluster.downing-provider.provider-key";
const FAILURE_DETECTOR_KEY: &str = ClusterCompatibilityKeyCatalog::FAILURE_DETECTOR.name();
const SINGLETON_KEY: &str = ClusterCompatibilityKeyCatalog::SINGLETON.name();
const SHARDING_STATE_STORE_MODE_KEY: &str = ClusterCompatibilityKeyCatalog::SHARDING_STATE_STORE_MODE.name();
const SBR_STABLE_AFTER_KEY: &str = "fraktor.cluster.downing-provider.split-brain-resolver.stable-after";
const SBR_ACTIVE_STRATEGY_KEY: &str = "fraktor.cluster.downing-provider.split-brain-resolver.active-strategy";
const SBR_DOWN_ALL_WHEN_UNSTABLE_KEY: &str =
  "fraktor.cluster.downing-provider.split-brain-resolver.down-all-when-unstable";
const REQUIRED_JOIN_COMPATIBILITY_KEYS: &[&str] = &[
  PUBSUB_SUBSCRIBER_TIMEOUT_KEY,
  PUBSUB_SUSPENDED_TTL_KEY,
  DOWNING_PROVIDER_KEY,
  FAILURE_DETECTOR_KEY,
  SINGLETON_KEY,
  SHARDING_STATE_STORE_MODE_KEY,
];
const CONDITIONAL_JOIN_COMPATIBILITY_KEYS: &[&str] =
  &[SBR_STABLE_AFTER_KEY, SBR_ACTIVE_STRATEGY_KEY, SBR_DOWN_ALL_WHEN_UNSTABLE_KEY];
const SENSITIVE_JOIN_COMPATIBILITY_KEYS: &[&str] = &[];

struct JoinCompatibilityCheck {
  key:             ClusterCompatibilityKey,
  mismatch_detail: fn(&ClusterExtensionConfig, &ClusterExtensionConfig) -> Option<String>,
}

impl JoinCompatibilityCheck {
  const fn new(
    key: ClusterCompatibilityKey,
    mismatch_detail: fn(&ClusterExtensionConfig, &ClusterExtensionConfig) -> Option<String>,
  ) -> Self {
    Self { key, mismatch_detail }
  }
}

const JOIN_COMPATIBILITY_CHECKS: &[JoinCompatibilityCheck] = &[
  JoinCompatibilityCheck::new(ClusterCompatibilityKeyCatalog::PUBSUB, pubsub_config_mismatch_detail),
  JoinCompatibilityCheck::new(ClusterCompatibilityKeyCatalog::DOWNING_PROVIDER, downing_provider_key_mismatch_detail),
  JoinCompatibilityCheck::new(
    ClusterCompatibilityKeyCatalog::SPLIT_BRAIN_RESOLVER_CONFIG,
    split_brain_resolver_config_mismatch_detail,
  ),
  JoinCompatibilityCheck::new(
    ClusterCompatibilityKeyCatalog::FAILURE_DETECTOR,
    failure_detector_config_mismatch_detail,
  ),
  JoinCompatibilityCheck::new(ClusterCompatibilityKeyCatalog::SINGLETON, singleton_config_mismatch_detail),
  JoinCompatibilityCheck::new(
    ClusterCompatibilityKeyCatalog::SHARDING_STATE_STORE_MODE,
    sharding_state_store_mode_mismatch_detail,
  ),
];

/// Configuration applied when installing the cluster extension.
#[derive(Clone, Debug, PartialEq)]
pub struct ClusterExtensionConfig {
  advertised_address: String,
  metrics_enabled: bool,
  static_topology: Option<ClusterTopology>,
  pubsub_config: PubSubConfig,
  failure_detector_config: FailureDetectorConfig,
  app_version: String,
  roles: Vec<String>,
  downing_provider: DowningProviderCompatibility,
  singleton_manager_config: ClusterSingletonManagerConfig,
  singleton_proxy_config: ClusterSingletonProxyConfig,
  sharding_state_store_mode: ClusterShardingStateStoreMode,
  grain_idle_passivation_threshold: Duration,
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
      metrics_enabled: false,
      static_topology: None,
      pubsub_config: PubSubConfig::new(Duration::from_secs(3), Duration::from_secs(60)),
      failure_detector_config: FailureDetectorConfig::new(),
      app_version: String::from(env!("CARGO_PKG_VERSION")),
      roles: Vec::new(),
      downing_provider: DowningProviderCompatibility::noop(),
      singleton_manager_config: ClusterSingletonManagerConfig::new(),
      singleton_proxy_config: ClusterSingletonProxyConfig::new(),
      sharding_state_store_mode: ClusterShardingStateStoreMode::default(),
      grain_idle_passivation_threshold: Duration::from_secs(3600),
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

  /// Sets the maximum Grain idle duration before passivation.
  #[must_use]
  pub const fn with_grain_idle_passivation_threshold(mut self, threshold: Duration) -> Self {
    self.grain_idle_passivation_threshold = threshold;
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

  /// Returns the maximum Grain idle duration before passivation.
  #[must_use]
  pub const fn grain_idle_passivation_threshold(&self) -> Duration {
    self.grain_idle_passivation_threshold
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

  /// Sets the failure detector configuration.
  #[must_use]
  pub const fn with_failure_detector_config(mut self, config: FailureDetectorConfig) -> Self {
    self.failure_detector_config = config;
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

  /// Returns the failure detector configuration.
  #[must_use]
  pub const fn failure_detector_config(&self) -> &FailureDetectorConfig {
    &self.failure_detector_config
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

  /// Returns non-sensitive configuration keys that must always match before accepting a join.
  #[must_use]
  pub const fn required_join_compatibility_keys() -> &'static [&'static str] {
    REQUIRED_JOIN_COMPATIBILITY_KEYS
  }

  /// Returns non-sensitive configuration keys that must match only when their provider is active.
  #[must_use]
  pub const fn conditional_join_compatibility_keys() -> &'static [&'static str] {
    CONDITIONAL_JOIN_COMPATIBILITY_KEYS
  }

  /// Returns configuration keys excluded from advertised join compatibility metadata.
  #[must_use]
  pub const fn sensitive_join_compatibility_keys() -> &'static [&'static str] {
    SENSITIVE_JOIN_COMPATIBILITY_KEYS
  }

  /// Returns whether the key participates in unconditional join compatibility checks.
  #[must_use]
  pub fn is_required_join_compatibility_key(key: &str) -> bool {
    REQUIRED_JOIN_COMPATIBILITY_KEYS.contains(&key)
  }

  /// Returns whether the key participates in provider-conditional join compatibility checks.
  #[must_use]
  pub fn is_conditional_join_compatibility_key(key: &str) -> bool {
    CONDITIONAL_JOIN_COMPATIBILITY_KEYS.contains(&key)
  }

  /// Returns whether the key must not be advertised in join compatibility metadata.
  #[must_use]
  pub fn is_sensitive_join_compatibility_key(key: &str) -> bool {
    SENSITIVE_JOIN_COMPATIBILITY_KEYS.contains(&key)
  }

  /// Sets the singleton manager configuration.
  #[must_use]
  pub fn with_singleton_manager_config(mut self, config: ClusterSingletonManagerConfig) -> Self {
    self.singleton_manager_config = config;
    self
  }

  /// Sets the singleton proxy configuration.
  #[must_use]
  pub fn with_singleton_proxy_config(mut self, config: ClusterSingletonProxyConfig) -> Self {
    self.singleton_proxy_config = config;
    self
  }

  /// Sets the sharding state-store mode advertised for join compatibility.
  #[must_use]
  pub const fn with_sharding_state_store_mode(mut self, mode: ClusterShardingStateStoreMode) -> Self {
    self.sharding_state_store_mode = mode;
    self
  }

  /// Returns the singleton manager configuration.
  #[must_use]
  pub const fn singleton_manager_config(&self) -> &ClusterSingletonManagerConfig {
    &self.singleton_manager_config
  }

  /// Returns the singleton proxy configuration.
  #[must_use]
  pub const fn singleton_proxy_config(&self) -> &ClusterSingletonProxyConfig {
    &self.singleton_proxy_config
  }

  /// Returns the sharding state-store mode advertised for join compatibility.
  #[must_use]
  pub const fn sharding_state_store_mode(&self) -> ClusterShardingStateStoreMode {
    self.sharding_state_store_mode
  }

  /// Validates singleton-related configuration values.
  ///
  /// # Errors
  ///
  /// Returns [`ClusterSingletonConfigError`] when the singleton manager or proxy configuration
  /// contains invalid values.
  pub fn validate_singleton(&self) -> Result<(), ClusterSingletonConfigError> {
    self.singleton_manager_config.validate()?;
    self.singleton_proxy_config.validate()
  }

  /// Validates cluster extension configuration values.
  ///
  /// # Errors
  ///
  /// Returns [`ClusterExtensionConfigError`] when a configured value is outside
  /// the accepted range.
  pub fn validate(&self) -> Result<(), ClusterExtensionConfigError> {
    self.failure_detector_config.validate()?;
    Self::validate_grain_idle_passivation_threshold(self.grain_idle_passivation_threshold)
  }

  pub(crate) fn validate_grain_idle_passivation_threshold(
    threshold: Duration,
  ) -> Result<(), ClusterExtensionConfigError> {
    if threshold < Duration::from_secs(1) {
      Err(ClusterExtensionConfigError::GrainIdlePassivationThresholdBelowOneSecond)
    } else {
      Ok(())
    }
  }
}

impl Default for ClusterExtensionConfig {
  fn default() -> Self {
    Self::new()
  }
}

impl JoinConfigCompatChecker for ClusterExtensionConfig {
  fn check_join_compatibility(&self, joining: &ClusterExtensionConfig) -> ConfigValidation {
    let mut reason = String::new();

    for check in JOIN_COMPATIBILITY_CHECKS {
      if let Some(detail) = (check.mismatch_detail)(self, joining) {
        append_mismatch_reason(&mut reason, check.key, &detail);
      }
    }

    if reason.is_empty() { ConfigValidation::Compatible } else { ConfigValidation::Incompatible { reason } }
  }
}

fn append_mismatch_reason(reason: &mut String, key: ClusterCompatibilityKey, detail: &str) {
  if !reason.is_empty() {
    reason.push_str("; ");
  }
  reason.push_str(key.name());
  reason.push_str(" mismatch: ");
  reason.push_str(detail);
}

fn pubsub_configs_are_compatible(local: &ClusterExtensionConfig, joining: &ClusterExtensionConfig) -> bool {
  local.pubsub_config == joining.pubsub_config
}

fn pubsub_config_mismatch_detail(local: &ClusterExtensionConfig, joining: &ClusterExtensionConfig) -> Option<String> {
  if pubsub_configs_are_compatible(local, joining) {
    None
  } else {
    Some(String::from(PUBSUB_CONFIGURATION_MISMATCH_REASON))
  }
}

fn downing_provider_keys_are_compatible(local: &ClusterExtensionConfig, joining: &ClusterExtensionConfig) -> bool {
  local.downing_provider.provider_key() == joining.downing_provider.provider_key()
}

fn downing_provider_key_mismatch_detail(
  local: &ClusterExtensionConfig,
  joining: &ClusterExtensionConfig,
) -> Option<String> {
  if downing_provider_keys_are_compatible(local, joining) {
    None
  } else {
    Some(String::from(DOWNING_PROVIDER_KEY_MISMATCH_REASON))
  }
}

fn split_brain_resolver_config_are_compatible(
  local: &ClusterExtensionConfig,
  joining: &ClusterExtensionConfig,
) -> bool {
  local.downing_provider.provider_key() != SPLIT_BRAIN_RESOLVER_PROVIDER_KEY
    || joining.downing_provider.provider_key() != SPLIT_BRAIN_RESOLVER_PROVIDER_KEY
    || local.downing_provider.sbr_config_identity() == joining.downing_provider.sbr_config_identity()
}

fn split_brain_resolver_config_mismatch_detail(
  local: &ClusterExtensionConfig,
  joining: &ClusterExtensionConfig,
) -> Option<String> {
  if split_brain_resolver_config_are_compatible(local, joining) {
    None
  } else {
    Some(String::from(SBR_CONFIG_MISMATCH_REASON))
  }
}

fn failure_detector_config_mismatch_detail(
  local: &ClusterExtensionConfig,
  joining: &ClusterExtensionConfig,
) -> Option<String> {
  let field_names = local.failure_detector_config.difference_field_names(&joining.failure_detector_config);
  if field_names.is_empty() { None } else { Some(field_names.join(", ")) }
}

fn singleton_config_mismatch_detail(
  local: &ClusterExtensionConfig,
  joining: &ClusterExtensionConfig,
) -> Option<String> {
  let manager_fields: Vec<String> = local
    .singleton_manager_config
    .difference_field_names(&joining.singleton_manager_config)
    .into_iter()
    .map(|f| alloc::format!("manager.{f}"))
    .collect();
  let proxy_fields: Vec<String> = local
    .singleton_proxy_config
    .difference_field_names(&joining.singleton_proxy_config)
    .into_iter()
    .map(|f| alloc::format!("proxy.{f}"))
    .collect();
  let mut all_fields = manager_fields;
  all_fields.extend(proxy_fields);
  if all_fields.is_empty() { None } else { Some(all_fields.join(", ")) }
}

fn sharding_state_store_mode_mismatch_detail(
  local: &ClusterExtensionConfig,
  joining: &ClusterExtensionConfig,
) -> Option<String> {
  if local.sharding_state_store_mode == joining.sharding_state_store_mode {
    None
  } else {
    Some(String::from("state_store_mode"))
  }
}

fn normalize_roles(mut roles: Vec<String>) -> Vec<String> {
  roles.sort();
  roles.dedup();
  roles
}
