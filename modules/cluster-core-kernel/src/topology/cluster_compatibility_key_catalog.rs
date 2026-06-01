//! Immutable cluster compatibility key catalog.

use crate::topology::ClusterCompatibilityKey;

const LOCAL_ONLY_NODE_IDENTITY_REASON: &str = "local-only node identity is not compared during join compatibility";
const SENSITIVE_PROVIDER_FACTORY_REASON: &str =
  "sensitive local factory implementation is not compared during join compatibility";

static REQUIRED_KEYS: [ClusterCompatibilityKey; 2] =
  [ClusterCompatibilityKeyCatalog::PUBSUB, ClusterCompatibilityKeyCatalog::DOWNING_PROVIDER];

static EXCLUDED_KEYS: [ClusterCompatibilityKey; 2] =
  [ClusterCompatibilityKeyCatalog::ADVERTISED_ADDRESS, ClusterCompatibilityKeyCatalog::DOWNING_PROVIDER_FACTORY];

/// Catalog of stable cluster join compatibility keys.
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
  /// Pub/sub configuration compatibility key.
  pub const PUBSUB: ClusterCompatibilityKey = ClusterCompatibilityKey::required("cluster.pubsub");
  /// Split Brain Resolver settings compatibility key.
  pub const SPLIT_BRAIN_RESOLVER_SETTINGS: ClusterCompatibilityKey =
    ClusterCompatibilityKey::required("cluster.split-brain-resolver.settings");

  /// Returns required keys compared by join compatibility.
  #[must_use]
  pub const fn required_keys() -> &'static [ClusterCompatibilityKey] {
    &REQUIRED_KEYS
  }

  /// Returns keys excluded from join compatibility comparison.
  #[must_use]
  pub const fn excluded_keys() -> &'static [ClusterCompatibilityKey] {
    &EXCLUDED_KEYS
  }
}
