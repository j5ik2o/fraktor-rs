use fraktor_cluster_core_kernel_rs::activation::ClusterIdentityError;

use crate::{ClusterIdentity, GrainTypeKey};

#[derive(Debug, PartialEq)]
struct OrderMessage;

#[derive(Debug, PartialEq)]
struct UserMessage;

#[test]
fn identity_for_derives_same_value_as_cluster_identity_new() {
  // GrainTypeKey::identity_for の結果が ClusterIdentity::new(kind, entity_id) と同値であることを検証
  let key = GrainTypeKey::<UserMessage>::new("user");
  let derived = key.identity_for("alice").expect("identity_for should succeed");
  let direct = ClusterIdentity::<UserMessage>::new("user", "alice").expect("direct construction should succeed");
  assert_eq!(derived, direct);
}

#[test]
fn identity_for_rejects_empty_entity_id() {
  // 空 entity id は ClusterIdentityError::EmptyIdentity で拒否される
  let key = GrainTypeKey::<UserMessage>::new("user");
  let err = key.identity_for("").expect_err("empty entity id should fail");
  assert_eq!(err, ClusterIdentityError::EmptyIdentity);
}

#[test]
fn identity_for_rejects_empty_kind() {
  // 空 kind は ClusterIdentityError::EmptyKind で拒否される（identity_for で検証）
  let key = GrainTypeKey::<UserMessage>::new("");
  let err = key.identity_for("alice").expect_err("empty kind should fail");
  assert_eq!(err, ClusterIdentityError::EmptyKind);
}

#[test]
fn kind_returns_the_stored_kind() {
  let key = GrainTypeKey::<OrderMessage>::new("order");
  assert_eq!(key.kind(), "order");
}

#[test]
fn different_message_types_with_same_kind_and_entity_id_share_same_kernel_identity() {
  // M に依らず同一の kind + entity id は同一 kernel 宛先になる（要件 1.3）
  let key_user = GrainTypeKey::<UserMessage>::new("entity");
  let key_order = GrainTypeKey::<OrderMessage>::new("entity");
  let id_user = key_user.identity_for("42").expect("user identity");
  let id_order = key_order.identity_for("42").expect("order identity");
  assert_eq!(id_user.as_kernel(), id_order.as_kernel());
}
