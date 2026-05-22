use alloc::vec;

use crate::serialization::{
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

#[test]
fn rejects_trailing_bytes() {
  let message = SerializedMessage::new(id(100), Some("example.Manifest".into()), vec![1]);
  let mut encoded = message.encode();
  encoded.push(2);

  let error = SerializedMessage::decode(&encoded).expect_err("trailing bytes");
  assert_eq!(error, SerializationError::InvalidFormat);
}

#[test]
fn rejects_truncated_manifest_length() {
  let mut data = id(100).value().to_le_bytes().to_vec();
  data.push(1);
  data.extend_from_slice(&[1, 2]);

  let error = SerializedMessage::decode(&data).expect_err("truncated manifest length");
  assert_eq!(error, SerializationError::InvalidFormat);
}

#[test]
fn rejects_truncated_manifest_payload() {
  let mut data = id(100).value().to_le_bytes().to_vec();
  data.push(1);
  data.extend_from_slice(&4_u32.to_le_bytes());
  data.extend_from_slice(b"ab");

  let error = SerializedMessage::decode(&data).expect_err("truncated manifest payload");
  assert_eq!(error, SerializationError::InvalidFormat);
}

#[test]
fn rejects_truncated_payload_length() {
  let mut data = id(100).value().to_le_bytes().to_vec();
  data.push(0);
  data.extend_from_slice(&[1, 2]);

  let error = SerializedMessage::decode(&data).expect_err("truncated payload length");
  assert_eq!(error, SerializationError::InvalidFormat);
}

#[test]
fn rejects_end_offset_overflow() {
  let error = SerializedMessage::end_offset(&[], usize::MAX, 1).expect_err("overflow");

  assert_eq!(error, SerializationError::InvalidFormat);
}
