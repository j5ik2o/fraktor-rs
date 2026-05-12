use alloc::{string::String, vec};

use crate::{ClusterRouterGroup, cluster_router_group_config::ClusterRouterGroupConfig};

#[test]
fn group_settings_store_values() {
  let settings = ClusterRouterGroupConfig::new(vec![String::from("/user/a"), String::from("/user/b")])
    .with_allow_local_routees(false);
  assert_eq!(settings.routee_paths(), &[String::from("/user/a"), String::from("/user/b")]);
  assert!(!settings.allow_local_routees());
}

#[test]
fn group_settings_accept_empty_paths() {
  let settings = ClusterRouterGroupConfig::new(vec![]);
  assert!(settings.routee_paths().is_empty());
}

#[test]
fn empty_routee_paths_returns_none_for_key() {
  let settings = ClusterRouterGroupConfig::new(vec![]);
  let router = ClusterRouterGroup::new(settings);
  assert_eq!(router.routee_for_key(0), None);
  assert_eq!(router.routee_for_key(42), None);
}
