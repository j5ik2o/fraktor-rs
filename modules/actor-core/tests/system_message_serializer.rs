use core::any::{TypeId, type_name};

use fraktor_actor_core_rs::core::kernel::{
  actor::messaging::system_message::SystemMessage,
  serialization::{
    builtin::{SYSTEM_MESSAGE_ID, register_defaults},
    default_serialization_setup,
    serialization_registry::{SerializationRegistry, SerializerResolutionOrigin},
  },
};
use fraktor_utils_core_rs::core::sync::ArcShared;

#[test]
fn should_register_system_message_in_builtin_defaults() {
  let setup = default_serialization_setup();
  let registry = ArcShared::new(SerializationRegistry::from_setup(&setup));
  register_defaults(&registry, |name, id| panic!("unexpected builtin collision: {name} {id:?}"))
    .expect("builtin defaults should register");

  let (resolved, origin) = registry
    .serializer_for_type(TypeId::of::<SystemMessage>(), type_name::<SystemMessage>(), None)
    .expect("system message serializer should resolve");

  assert_eq!(resolved.identifier(), SYSTEM_MESSAGE_ID);
  assert_eq!(origin, SerializerResolutionOrigin::Binding);
  assert_eq!(registry.binding_name(TypeId::of::<SystemMessage>()).as_deref(), Some("SystemMessage"));
}
