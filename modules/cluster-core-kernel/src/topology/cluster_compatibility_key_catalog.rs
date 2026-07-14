//! Immutable cluster compatibility key catalog.

#[cfg(test)]
#[path = "cluster_compatibility_key_catalog_test.rs"]
mod tests;

use crate::topology::ClusterCompatibilityKey;

const LOCAL_ONLY_NODE_IDENTITY_REASON: &str = "local-only node identity is not compared during join compatibility";
const SENSITIVE_PROVIDER_FACTORY_REASON: &str =
  "sensitive local factory implementation is not compared during join compatibility";
const UNOWNED_FAILURE_DETECTOR_CHOICE_REASON: &str =
  "failure detector implementation choice is not compared until cluster config owns detector selection";
const SHARDING_IDENTITY_LOOKUP_CHOICE_REASON: &str = "sharding identity lookup implementation choice is factory-injected and not owned by config, so it is not compared during join compatibility";
const SHARDING_IDENTITY_LOOKUP_TUNING_REASON: &str =
  "sharding identity lookup tuning values are local-only and need not match across nodes during join compatibility";

static REQUIRED_KEYS: [ClusterCompatibilityKey; 6] = [
  ClusterCompatibilityKeyCatalog::PUBSUB,
  ClusterCompatibilityKeyCatalog::DOWNING_PROVIDER,
  ClusterCompatibilityKeyCatalog::FAILURE_DETECTOR,
  ClusterCompatibilityKeyCatalog::SINGLETON,
  ClusterCompatibilityKeyCatalog::SHARDING_STATE_STORE_MODE,
  ClusterCompatibilityKeyCatalog::GRAIN_IDLE_PASSIVATION_THRESHOLD,
];

static CONDITIONAL_KEYS: [ClusterCompatibilityKey; 1] = [ClusterCompatibilityKeyCatalog::SPLIT_BRAIN_RESOLVER_CONFIG];

static EXCLUDED_KEYS: [ClusterCompatibilityKey; 5] = [
  ClusterCompatibilityKeyCatalog::ADVERTISED_ADDRESS,
  ClusterCompatibilityKeyCatalog::DOWNING_PROVIDER_FACTORY,
  ClusterCompatibilityKeyCatalog::FAILURE_DETECTOR_CHOICE,
  ClusterCompatibilityKeyCatalog::SHARDING_IDENTITY_LOOKUP_CHOICE,
  ClusterCompatibilityKeyCatalog::SHARDING_IDENTITY_LOOKUP_TUNING,
];

/// Catalog of stable cluster join compatibility keys.
///
/// Grain identity lookup settings remain excluded because they are factory-injected
/// or local-only. Runtime settings such as idle passivation are required when a
/// mismatch would change Grain lifecycle behavior across nodes.
pub struct ClusterCompatibilityKeyCatalog;

impl ClusterCompatibilityKeyCatalog {
  /// Advertised address key excluded because it is local node identity.
  pub const ADVERTISED_ADDRESS: ClusterCompatibilityKey =
    ClusterCompatibilityKey::excluded("cluster.advertised-address", LOCAL_ONLY_NODE_IDENTITY_REASON);
  /// Downing provider compatibility key.
  pub const DOWNING_PROVIDER: ClusterCompatibilityKey = ClusterCompatibilityKey::required("cluster.downing-provider");
  /// Downing provider factory key excluded because implementation identity is local and sensitive.
  pub const DOWNING_PROVIDER_FACTORY: ClusterCompatibilityKey =
    ClusterCompatibilityKey::excluded("cluster.downing-provider.factory", SENSITIVE_PROVIDER_FACTORY_REASON);
  /// Failure detector configuration compatibility key.
  pub const FAILURE_DETECTOR: ClusterCompatibilityKey = ClusterCompatibilityKey::required("cluster.failure-detector");
  /// Failure detector implementation choice vocabulary key.
  pub const FAILURE_DETECTOR_CHOICE: ClusterCompatibilityKey =
    ClusterCompatibilityKey::excluded("cluster.failure-detector.choice", UNOWNED_FAILURE_DETECTOR_CHOICE_REASON);
  /// Grain idle-passivation threshold compatibility key.
  pub const GRAIN_IDLE_PASSIVATION_THRESHOLD: ClusterCompatibilityKey =
    ClusterCompatibilityKey::required("cluster.grain.idle-passivation-threshold");
  /// Pub/sub configuration compatibility key.
  pub const PUBSUB: ClusterCompatibilityKey = ClusterCompatibilityKey::required("cluster.pubsub");
  /// Sharding identity lookup implementation choice key excluded because it is factory-injected and
  /// not owned by config.
  pub const SHARDING_IDENTITY_LOOKUP_CHOICE: ClusterCompatibilityKey = ClusterCompatibilityKey::excluded(
    "cluster.sharding.identity-lookup.choice",
    SHARDING_IDENTITY_LOOKUP_CHOICE_REASON,
  );
  /// Sharding identity lookup tuning key excluded because the values are local-only.
  pub const SHARDING_IDENTITY_LOOKUP_TUNING: ClusterCompatibilityKey = ClusterCompatibilityKey::excluded(
    "cluster.sharding.identity-lookup.tuning",
    SHARDING_IDENTITY_LOOKUP_TUNING_REASON,
  );
  /// Sharding state-store mode compatibility key.
  pub const SHARDING_STATE_STORE_MODE: ClusterCompatibilityKey =
    ClusterCompatibilityKey::required("cluster.sharding.state-store-mode");
  /// Singleton configuration compatibility key.
  pub const SINGLETON: ClusterCompatibilityKey = ClusterCompatibilityKey::required("cluster.singleton");
  /// Split Brain Resolver config compatibility key.
  pub const SPLIT_BRAIN_RESOLVER_CONFIG: ClusterCompatibilityKey =
    ClusterCompatibilityKey::required("cluster.split-brain-resolver.config");

  /// Returns required keys compared by join compatibility.
  #[must_use]
  pub const fn required_keys() -> &'static [ClusterCompatibilityKey] {
    &REQUIRED_KEYS
  }

  /// Returns keys compared only when their provider is active.
  #[must_use]
  pub const fn conditional_keys() -> &'static [ClusterCompatibilityKey] {
    &CONDITIONAL_KEYS
  }

  /// Returns keys excluded from join compatibility comparison.
  #[must_use]
  pub const fn excluded_keys() -> &'static [ClusterCompatibilityKey] {
    &EXCLUDED_KEYS
  }
}
