use core::any::{TypeId, type_name};

use fraktor_actor_core_rs::core::kernel::{
  actor::{Pid, messaging::system_message::SystemMessage},
  serialization::{
    SerializationError, Serializer,
    builtin::{SYSTEM_MESSAGE_ID, SystemMessageSerializer, register_defaults},
    default_serialization_setup,
    serialization_registry::{SerializationRegistry, SerializerResolutionOrigin},
  },
};
use fraktor_utils_core_rs::core::sync::ArcShared;

fn serializer() -> SystemMessageSerializer {
  SystemMessageSerializer::new(SYSTEM_MESSAGE_ID)
}

fn round_trip(message: SystemMessage) -> SystemMessage {
  let serializer = serializer();
  let bytes = serializer.to_binary(&message).expect("system message should encode");
  decode_system_message(&serializer, &bytes)
}

fn decode_system_message(serializer: &SystemMessageSerializer, bytes: &[u8]) -> SystemMessage {
  let decoded = serializer.from_binary(bytes, None).expect("system message should decode");
  *decoded.downcast::<SystemMessage>().expect("decoded payload should be SystemMessage")
}

#[test]
fn should_return_configured_serializer_id() {
  let serializer = serializer();
  assert_eq!(serializer.identifier(), SYSTEM_MESSAGE_ID);
}

#[test]
fn should_not_require_manifest() {
  let serializer = serializer();
  assert!(!serializer.include_manifest());
}

#[test]
fn should_round_trip_stop_as_terminate_equivalent() {
  let decoded = round_trip(SystemMessage::Stop);
  assert_eq!(decoded, SystemMessage::Stop);
}

#[test]
fn should_round_trip_watch_with_pid_generation() {
  let pid = Pid::new(42, 7);
  let decoded = round_trip(SystemMessage::Watch(pid));
  assert_eq!(decoded, SystemMessage::Watch(pid));
}

#[test]
fn should_round_trip_unwatch_with_pid_generation() {
  let pid = Pid::new(43, 8);
  let decoded = round_trip(SystemMessage::Unwatch(pid));
  assert_eq!(decoded, SystemMessage::Unwatch(pid));
}

#[test]
fn should_round_trip_death_watch_notification_with_pid_generation() {
  let pid = Pid::new(44, 9);
  let decoded = round_trip(SystemMessage::DeathWatchNotification(pid));
  assert_eq!(decoded, SystemMessage::DeathWatchNotification(pid));
}

#[test]
fn should_encode_pid_payload_as_little_endian_after_tag() {
  let serializer = serializer();
  let pid = Pid::new(0x0102_0304_0506_0708, 0x0a0b_0c0d);
  let bytes = serializer.to_binary(&SystemMessage::Watch(pid)).expect("watch should encode");

  assert_eq!(bytes.len(), 13);
  assert_eq!(&bytes[1..9], &pid.value().to_le_bytes());
  assert_eq!(&bytes[9..13], &pid.generation().to_le_bytes());
}

#[test]
fn should_reject_unsupported_system_message_variant() {
  let serializer = serializer();
  let unsupported = SystemMessage::StopChild(Pid::new(45, 10));
  let Err(error) = serializer.to_binary(&unsupported) else { panic!("unsupported system message should fail") };
  assert!(matches!(error, SerializationError::InvalidFormat));
}

#[test]
fn should_reject_non_system_message_type() {
  let serializer = serializer();
  let wrong_type = 123_i32;
  let Err(error) = serializer.to_binary(&wrong_type) else { panic!("non system message should fail") };
  assert!(matches!(error, SerializationError::InvalidFormat));
}

#[test]
fn should_reject_unknown_wire_tag() {
  let serializer = serializer();
  let Err(error) = serializer.from_binary(&[u8::MAX], None) else { panic!("unknown tag should fail") };
  assert!(matches!(error, SerializationError::InvalidFormat));
}

#[test]
fn should_reject_truncated_pid_payload() {
  let serializer = serializer();
  let mut bytes = serializer.to_binary(&SystemMessage::Watch(Pid::new(46, 11))).expect("watch should encode");
  bytes.truncate(bytes.len() - 1);

  let Err(error) = serializer.from_binary(&bytes, None) else { panic!("truncated pid should fail") };
  assert!(matches!(error, SerializationError::InvalidFormat));
}

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
