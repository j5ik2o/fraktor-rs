use alloc::{string::String, vec};

use crate::extension::{ClusterRouterGroup, ClusterRouterGroupConfig};

#[test]
fn routee_for_key_maps_consistently() {
  let config =
    ClusterRouterGroupConfig::new(vec![String::from("/user/a"), String::from("/user/b"), String::from("/user/c")]);
  let router = ClusterRouterGroup::new(config);

  assert_eq!(router.routee_for_key(0), Some("/user/a"));
  assert_eq!(router.routee_for_key(1), Some("/user/b"));
  assert_eq!(router.routee_for_key(2), Some("/user/c"));
  assert_eq!(router.routee_for_key(3), Some("/user/a"));
}

#[test]
fn local_routee_paths_returns_paths_when_local_allowed_and_roles_match() {
  let config = ClusterRouterGroupConfig::new(vec![String::from("/user/a"), String::from("/user/b")])
    .with_use_roles(vec![String::from("backend")]);
  let router = ClusterRouterGroup::new(config);
  // The local node carries the required role (plus extras), so it participates.
  assert_eq!(router.local_routee_paths(&[String::from("backend"), String::from("web")]), &[
    String::from("/user/a"),
    String::from("/user/b")
  ]);
}

#[test]
fn local_routee_paths_empty_when_local_routees_disallowed() {
  let config = ClusterRouterGroupConfig::new(vec![String::from("/user/a")]).with_allow_local_routees(false);
  let router = ClusterRouterGroup::new(config);
  assert!(router.local_routee_paths(&[]).is_empty());
}

#[test]
fn local_routee_paths_empty_when_roles_not_satisfied() {
  let config =
    ClusterRouterGroupConfig::new(vec![String::from("/user/a")]).with_use_roles(vec![String::from("backend")]);
  let router = ClusterRouterGroup::new(config);
  assert!(router.local_routee_paths(&[String::from("web")]).is_empty());
}

#[test]
fn local_routee_paths_returns_paths_with_no_role_requirement() {
  let config = ClusterRouterGroupConfig::new(vec![String::from("/user/a")]);
  let router = ClusterRouterGroup::new(config);
  // Default config allows local routees and requires no roles.
  assert_eq!(router.local_routee_paths(&[]), &[String::from("/user/a")]);
}
