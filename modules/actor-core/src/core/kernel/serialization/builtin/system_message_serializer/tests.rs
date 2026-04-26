use super::SystemMessageSerializer;
use crate::core::kernel::{
  actor::{Pid, messaging::system_message::SystemMessage},
  serialization::{builtin::SYSTEM_MESSAGE_ID, error::SerializationError, serializer::Serializer},
};

fn serializer() -> SystemMessageSerializer {
  SystemMessageSerializer::new(SYSTEM_MESSAGE_ID)
}

fn round_trip(message: SystemMessage) -> SystemMessage {
  let serializer = serializer();
  let bytes = serializer.to_binary(&message).expect("system message should encode");
  let decoded = serializer.from_binary(&bytes, None).expect("system message should decode");
  *decoded.downcast::<SystemMessage>().expect("decoded payload should be SystemMessage")
}

#[test]
fn identifier_returns_configured_id() {
  assert_eq!(serializer().identifier(), SYSTEM_MESSAGE_ID);
}

#[test]
fn include_manifest_is_false() {
  assert!(!serializer().include_manifest());
}

#[test]
fn stop_round_trips_as_parameterless_message() {
  assert_eq!(round_trip(SystemMessage::Stop), SystemMessage::Stop);
}

#[test]
fn watch_round_trips_with_pid_generation() {
  let pid = Pid::new(42, 7);
  assert_eq!(round_trip(SystemMessage::Watch(pid)), SystemMessage::Watch(pid));
}

#[test]
fn unwatch_round_trips_with_pid_generation() {
  let pid = Pid::new(43, 8);
  assert_eq!(round_trip(SystemMessage::Unwatch(pid)), SystemMessage::Unwatch(pid));
}

#[test]
fn death_watch_notification_round_trips_with_pid_generation() {
  let pid = Pid::new(44, 9);
  assert_eq!(round_trip(SystemMessage::DeathWatchNotification(pid)), SystemMessage::DeathWatchNotification(pid));
}

#[test]
fn pid_payload_uses_little_endian_layout_after_tag() {
  let pid = Pid::new(0x0102_0304_0506_0708, 0x0a0b_0c0d);
  let bytes = serializer().to_binary(&SystemMessage::Watch(pid)).expect("watch should encode");

  assert_eq!(bytes.len(), 13);
  assert_eq!(&bytes[1..9], &pid.value().to_le_bytes());
  assert_eq!(&bytes[9..13], &pid.generation().to_le_bytes());
}

#[test]
fn unsupported_system_message_variant_is_rejected() {
  let unsupported = SystemMessage::StopChild(Pid::new(45, 10));
  let result = serializer().to_binary(&unsupported);
  assert!(matches!(result, Err(SerializationError::InvalidFormat)));
}

#[test]
fn non_system_message_type_is_rejected() {
  let wrong_type = 123_i32;
  let result = serializer().to_binary(&wrong_type);
  assert!(matches!(result, Err(SerializationError::InvalidFormat)));
}

#[test]
fn unknown_wire_tag_is_rejected() {
  let result = serializer().from_binary(&[u8::MAX], None);
  assert!(matches!(result, Err(SerializationError::InvalidFormat)));
}

#[test]
fn truncated_pid_payload_is_rejected() {
  let mut bytes = serializer().to_binary(&SystemMessage::Watch(Pid::new(46, 11))).expect("watch should encode");
  bytes.truncate(bytes.len() - 1);

  let result = serializer().from_binary(&bytes, None);
  assert!(matches!(result, Err(SerializationError::InvalidFormat)));
}

#[test]
fn stop_payload_with_extra_bytes_is_rejected() {
  let result = serializer().from_binary(&[1, 0], None);
  assert!(matches!(result, Err(SerializationError::InvalidFormat)));
}
