use alloc::vec::Vec;
use core::any::TypeId;

use fraktor_utils_core_rs::sync::ArcShared;

use super::{
  super::{default_serialization_setup, serialization_registry::SerializationRegistry, serializer_id::SerializerId},
  MISC_MESSAGE_ID, NullSerializer, register_defaults,
};
use crate::{
  actor::{
    deploy::RemoteScope,
    messaging::{ActorIdentity, Status},
  },
  routing::RemoteRouterConfig,
};

#[test]
fn register_defaults_binds_remote_router_config_once() {
  let setup = default_serialization_setup();
  let registry = ArcShared::new(SerializationRegistry::from_setup(&setup));

  register_defaults(&registry, |_name, _id| {}).expect("register_defaults");

  assert_eq!(registry.binding_name(TypeId::of::<RemoteRouterConfig>()).as_deref(), Some("RemoteRouterConfig"));
}

#[test]
fn register_defaults_does_not_register_misc_bindings_when_misc_serializer_id_collides() {
  let setup = default_serialization_setup();
  let registry = ArcShared::new(SerializationRegistry::from_setup(&setup));
  // 先に MISC_MESSAGE_ID を別 serializer が占有している状態を作る。
  assert!(registry.register_serializer(MISC_MESSAGE_ID, ArcShared::new(NullSerializer::new(MISC_MESSAGE_ID))));

  let mut collisions: Vec<(&'static str, SerializerId)> = Vec::new();
  register_defaults(&registry, |name, id| collisions.push((name, id))).expect("register_defaults");

  assert!(
    collisions.iter().any(|(name, id)| *name == "misc_message" && *id == MISC_MESSAGE_ID),
    "misc_message collision must surface to the on_collision callback"
  );
  // 衝突した場合 ActorIdentity / RemoteScope / RemoteRouterConfig / Status の追加 binding を
  // MISC_MESSAGE_ID に固定登録してはならない。
  assert!(registry.binding_name(TypeId::of::<ActorIdentity>()).is_none(), "ActorIdentity must not bind on collision");
  assert!(registry.binding_name(TypeId::of::<RemoteScope>()).is_none(), "RemoteScope must not bind on collision");
  assert!(
    registry.binding_name(TypeId::of::<RemoteRouterConfig>()).is_none(),
    "RemoteRouterConfig must not bind on collision"
  );
  assert!(registry.binding_name(TypeId::of::<Status>()).is_none(), "Status must not bind on collision");
}
