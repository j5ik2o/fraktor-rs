use std::collections::HashSet;

use crate::core::{ClusterIdentity, ClusterIdentityError};

#[test]
fn new_rejects_empty_kind() {
  let err = ClusterIdentity::new("", "id").expect_err("empty kind should fail");
  assert_eq!(err, ClusterIdentityError::EmptyKind);
}

#[test]
fn new_rejects_empty_identity() {
  let err = ClusterIdentity::new("kind", "").expect_err("empty identity should fail");
  assert_eq!(err, ClusterIdentityError::EmptyIdentity);
}

#[test]
fn key_builds_kind_identity_string() {
  let identity = ClusterIdentity::new("kind", "id").expect("identity");
  let key = identity.key();
  assert_eq!(key.value(), "kind/id");
}

#[test]
fn identity_equality_and_hash_are_stable() {
  let first = ClusterIdentity::new("kind", "id").expect("identity");
  let second = ClusterIdentity::new("kind", "id").expect("identity");
  let third = ClusterIdentity::new("kind", "other").expect("identity");

  assert_eq!(first, second);
  assert_ne!(first, third);

  let mut set = HashSet::new();
  set.insert(first);
  assert!(set.contains(&second));
  assert!(!set.contains(&third));
}
