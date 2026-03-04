use alloc::{string::String, vec};

use crate::core::{ClusterRouterGroup, cluster_router_group_settings::ClusterRouterGroupSettings};

#[test]
fn group_settings_store_values() {
  let settings = ClusterRouterGroupSettings::new(vec![String::from("/user/a"), String::from("/user/b")])
    .with_allow_local_routees(false);
  assert_eq!(settings.routee_paths(), &[String::from("/user/a"), String::from("/user/b")]);
  assert!(!settings.allow_local_routees());
}

#[test]
fn group_settings_accept_empty_paths() {
  let settings = ClusterRouterGroupSettings::new(vec![]);
  assert!(settings.routee_paths().is_empty());
}

#[test]
fn empty_routee_paths_returns_none_for_key() {
  let settings = ClusterRouterGroupSettings::new(vec![]);
  let router = ClusterRouterGroup::new(settings);
  assert_eq!(router.routee_for_key(0), None);
  assert_eq!(router.routee_for_key(42), None);
}
