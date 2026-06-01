use crate::topology::{ClusterCompatibilityKey, ClusterCompatibilityKeyCatalog, ClusterCompatibilityKeySet};

const REQUIRED_KEYS: &[ClusterCompatibilityKey] = ClusterCompatibilityKeyCatalog::required_keys();
const EXCLUDED_KEYS: &[ClusterCompatibilityKey] = ClusterCompatibilityKeyCatalog::excluded_keys();

#[test]
fn key_set_uses_static_catalog_slices() {
  let catalog = ClusterCompatibilityKeySet::cluster_compatibility();

  assert_eq!(catalog.required_keys(), REQUIRED_KEYS);
  assert_eq!(catalog.excluded_keys(), EXCLUDED_KEYS);
}
