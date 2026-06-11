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
