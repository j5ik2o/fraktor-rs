use super::ByteStringSerializer;
use crate::core::kernel::{
  serialization::{builtin::BYTE_STRING_ID, error::SerializationError, serializer::Serializer},
  support::ByteString,
};

#[test]
fn round_trip_preserves_content() {
  let serializer = ByteStringSerializer::new(BYTE_STRING_ID);
  let original = ByteString::from_slice(&[1, 2, 3, 4, 5]);
  let bytes = serializer.to_binary(&original).expect("to_binary");
  let decoded = serializer.from_binary(&bytes, None).expect("from_binary");
  let result = decoded.downcast::<ByteString>().expect("downcast");
  assert_eq!(result.as_slice(), &[1, 2, 3, 4, 5]);
}

#[test]
fn round_trip_empty_byte_string() {
  let serializer = ByteStringSerializer::new(BYTE_STRING_ID);
  let original = ByteString::empty();
  let bytes = serializer.to_binary(&original).expect("to_binary");
  assert!(bytes.is_empty());
  let decoded = serializer.from_binary(&bytes, None).expect("from_binary");
  let result = decoded.downcast::<ByteString>().expect("downcast");
  assert!(result.is_empty());
}

#[test]
fn to_binary_rejects_non_byte_string_type() {
  let serializer = ByteStringSerializer::new(BYTE_STRING_ID);
  let wrong_type: i32 = 42;
  let result = serializer.to_binary(&wrong_type);
  assert!(matches!(result, Err(SerializationError::InvalidFormat)));
}

#[test]
fn identifier_returns_configured_id() {
  let serializer = ByteStringSerializer::new(BYTE_STRING_ID);
  assert_eq!(serializer.identifier(), BYTE_STRING_ID);
}
