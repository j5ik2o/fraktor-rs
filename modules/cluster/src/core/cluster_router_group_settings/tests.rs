use alloc::{string::String, vec};

use crate::core::cluster_router_group_settings::ClusterRouterGroupSettings;

#[test]
fn group_settings_store_values() {
  let settings = ClusterRouterGroupSettings::new(vec![String::from("/user/a"), String::from("/user/b")])
    .with_allow_local_routees(false);
  assert_eq!(settings.routee_paths(), &[String::from("/user/a"), String::from("/user/b")]);
  assert!(!settings.allow_local_routees());
}

#[test]
#[should_panic(expected = "routee paths must not be empty")]
fn group_settings_reject_empty_paths() {
  let _ = ClusterRouterGroupSettings::new(vec![]);
}
