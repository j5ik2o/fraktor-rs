use alloc::{string::String, vec};

use crate::core::{ClusterRouterGroup, ClusterRouterGroupSettings};

#[test]
fn routee_for_key_maps_consistently() {
  let settings =
    ClusterRouterGroupSettings::new(vec![String::from("/user/a"), String::from("/user/b"), String::from("/user/c")]);
  let router = ClusterRouterGroup::new(settings);

  assert_eq!(router.routee_for_key(0), Some("/user/a"));
  assert_eq!(router.routee_for_key(1), Some("/user/b"));
  assert_eq!(router.routee_for_key(2), Some("/user/c"));
  assert_eq!(router.routee_for_key(3), Some("/user/a"));
}
