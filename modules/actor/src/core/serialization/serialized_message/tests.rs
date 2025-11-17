use alloc::vec;

use crate::core::serialization::{
  error::SerializationError, serialized_message::SerializedMessage, serializer_id::SerializerId,
};

fn id(value: u32) -> SerializerId {
  SerializerId::try_from(value).expect("valid id")
}

#[test]
fn round_trip_without_manifest() {
  let message = SerializedMessage::new(id(100), None, vec![1, 2, 3]);
  let encoded = message.encode();
  let decoded = SerializedMessage::decode(&encoded).expect("decode");
  assert_eq!(decoded.serializer_id(), id(100));
  assert_eq!(decoded.manifest(), None);
  assert_eq!(decoded.bytes(), &[1, 2, 3]);
}

#[test]
fn round_trip_with_manifest() {
  let message = SerializedMessage::new(id(101), Some("example.Manifest".into()), vec![9, 8, 7]);
  let encoded = message.encode();
  let decoded = SerializedMessage::decode(&encoded).expect("decode");
  assert_eq!(decoded.manifest(), Some("example.Manifest"));
  assert_eq!(decoded.bytes(), &[9, 8, 7]);
}

#[test]
fn rejects_invalid_format() {
  let data = vec![0_u8; 3];
  let error = SerializedMessage::decode(&data).expect_err("invalid format");
  assert_eq!(error, SerializationError::InvalidFormat);
}
