use crate::topology::cluster_compatibility_key_catalog::ClusterCompatibilityKeyCatalog;

#[test]
fn singleton_key_name_is_cluster_singleton() {
  assert_eq!(ClusterCompatibilityKeyCatalog::SINGLETON.name(), "cluster.singleton");
}

#[test]
fn singleton_key_is_in_required_keys() {
  let required = ClusterCompatibilityKeyCatalog::required_keys();
  assert!(
    required.iter().any(|k| k.name() == "cluster.singleton"),
    "cluster.singleton が required_keys() に含まれていない"
  );
}

#[test]
fn sharding_state_store_mode_key_is_required() {
  let required = ClusterCompatibilityKeyCatalog::required_keys();
  assert!(
    required.iter().any(|k| k.name() == "cluster.sharding.state-store-mode"),
    "cluster.sharding.state-store-mode が required_keys() に含まれていない"
  );
}

#[test]
fn grain_idle_passivation_threshold_key_is_required() {
  let required = ClusterCompatibilityKeyCatalog::required_keys();
  assert!(
    required.iter().any(|k| k.name() == "cluster.grain.idle-passivation-threshold"),
    "cluster.grain.idle-passivation-threshold が required_keys() に含まれていない"
  );
}

#[test]
fn sharding_identity_lookup_keys_have_stable_names() {
  assert_eq!(
    ClusterCompatibilityKeyCatalog::SHARDING_IDENTITY_LOOKUP_CHOICE.name(),
    "cluster.sharding.identity-lookup.choice"
  );
  assert_eq!(
    ClusterCompatibilityKeyCatalog::SHARDING_IDENTITY_LOOKUP_TUNING.name(),
    "cluster.sharding.identity-lookup.tuning"
  );
}

#[test]
fn sharding_identity_lookup_keys_are_excluded_with_reason() {
  let excluded = ClusterCompatibilityKeyCatalog::excluded_keys();
  for name in ["cluster.sharding.identity-lookup.choice", "cluster.sharding.identity-lookup.tuning"] {
    let key = excluded
      .iter()
      .find(|k| k.name() == name)
      .unwrap_or_else(|| panic!("{name} が excluded_keys() に含まれていない"));
    assert!(key.exclusion_reason().is_some(), "{name} に除外理由が設定されていない");
  }
}

#[test]
fn sharding_identity_lookup_keys_are_not_compared() {
  let required = ClusterCompatibilityKeyCatalog::required_keys();
  let conditional = ClusterCompatibilityKeyCatalog::conditional_keys();
  for name in ["cluster.sharding.identity-lookup.choice", "cluster.sharding.identity-lookup.tuning"] {
    assert!(!required.iter().any(|k| k.name() == name), "{name} が required_keys() に混入している");
    assert!(!conditional.iter().any(|k| k.name() == name), "{name} が conditional_keys() に混入している");
  }
}
