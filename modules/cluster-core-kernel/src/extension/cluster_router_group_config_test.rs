use alloc::{string::String, vec};

use crate::extension::{ClusterRouterGroup, ClusterRouterGroupConfig};

#[test]
fn group_settings_store_values() {
  let settings = ClusterRouterGroupConfig::new(vec![String::from("/user/a"), String::from("/user/b")])
    .with_allow_local_routees(false)
    .with_use_roles(vec![String::from("frontend"), String::from("backend"), String::from("backend")]);
  assert_eq!(settings.routee_paths(), &[String::from("/user/a"), String::from("/user/b")]);
  assert!(!settings.allow_local_routees());
  assert_eq!(settings.use_roles(), &[String::from("backend"), String::from("frontend")]);
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

#[test]
fn group_settings_default_to_no_roles() {
  let settings = ClusterRouterGroupConfig::new(vec![]);
  assert!(settings.use_roles().is_empty());
}

#[test]
fn with_use_roles_stores_roles() {
  let settings = ClusterRouterGroupConfig::new(vec![]).with_use_roles(vec![String::from("backend")]);
  assert_eq!(settings.use_roles(), &[String::from("backend")]);
}

#[test]
fn satisfies_roles_requires_all_configured_roles() {
  let settings =
    ClusterRouterGroupConfig::new(vec![]).with_use_roles(vec![String::from("backend"), String::from("compute")]);
  assert!(settings.satisfies_roles(&[String::from("backend"), String::from("compute")]));
  assert!(!settings.satisfies_roles(&[String::from("compute")]));
  // A node carrying no roles never satisfies a non-empty requirement.
  assert!(!settings.satisfies_roles(&[]));
}

#[test]
fn satisfies_roles_with_empty_requirement_matches_any_node() {
  let settings = ClusterRouterGroupConfig::new(vec![]);
  assert!(settings.satisfies_roles(&[]));
  assert!(settings.satisfies_roles(&[String::from("anything")]));
}
