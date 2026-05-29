use fraktor_cluster_core_kernel_rs::activation::{ClusterIdentity as KernelClusterIdentity, ClusterIdentityError};

use crate::ClusterIdentity;

#[derive(Debug)]
struct OrderMessage;

#[derive(Debug)]
struct UserMessage;

#[test]
fn new_reuses_kernel_validation() {
  let empty_kind = ClusterIdentity::<UserMessage>::new("", "id").expect_err("empty kind should fail");
  assert_eq!(empty_kind, ClusterIdentityError::EmptyKind);

  let empty_identity = ClusterIdentity::<UserMessage>::new("kind", "").expect_err("empty identity should fail");
  assert_eq!(empty_identity, ClusterIdentityError::EmptyIdentity);
}

#[test]
fn converts_to_kernel_identity() {
  let typed = ClusterIdentity::<UserMessage>::new("user", "alice").expect("typed identity");
  let kernel: KernelClusterIdentity = typed.into();

  assert_eq!(kernel.kind(), "user");
  assert_eq!(kernel.identity(), "alice");
}

#[test]
fn wraps_kernel_identity() {
  let kernel = KernelClusterIdentity::new("order", "42").expect("kernel identity");
  let typed = ClusterIdentity::<OrderMessage>::from_kernel(kernel);

  assert_eq!(typed.kind(), "order");
  assert_eq!(typed.identity(), "42");
}

#[test]
fn carries_message_type_marker() {
  fn accept_user_identity(_identity: ClusterIdentity<UserMessage>) {}

  let typed = ClusterIdentity::<UserMessage>::new("user", "alice").expect("typed identity");

  accept_user_identity(typed);
}
