use crate::topology::ClusterCompatibilityKeyCatalog;

#[test]
fn catalog_exposes_required_stable_key_names() {
  let required_names: Vec<_> = ClusterCompatibilityKeyCatalog::required_keys().iter().map(|key| key.name()).collect();

  assert_eq!(required_names, vec!["cluster.pubsub", "cluster.downing-provider"]);
}

#[test]
fn catalog_exposes_conditional_stable_key_names() {
  let conditional_names: Vec<_> =
    ClusterCompatibilityKeyCatalog::conditional_keys().iter().map(|key| key.name()).collect();

  assert_eq!(conditional_names, vec!["cluster.split-brain-resolver.settings"]);
}

#[test]
fn catalog_exposes_excluded_keys_with_reasons() {
  let excluded = ClusterCompatibilityKeyCatalog::excluded_keys();

  assert!(excluded.iter().any(|key| {
    key.name() == "cluster.advertised-address"
      && key.exclusion_reason() == Some("local-only node identity is not compared during join compatibility")
  }));
  assert!(excluded.iter().any(|key| {
    key.name() == "cluster.downing-provider.factory"
      && key.exclusion_reason()
        == Some("sensitive local factory implementation is not compared during join compatibility")
  }));
  assert!(excluded.iter().all(|key| key.exclusion_reason().is_some()));
}
