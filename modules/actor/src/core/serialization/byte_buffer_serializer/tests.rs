use alloc::{boxed::Box, string::String, vec, vec::Vec};
use core::any::Any;

use super::ByteBufferSerializer;
use crate::core::serialization::error::SerializationError;

/// Minimal implementation for testing.
struct StubByteBufferSerializer;

impl ByteBufferSerializer for StubByteBufferSerializer {
  fn to_binary_buf(&self, message: &(dyn Any + Send + Sync), buf: &mut Vec<u8>) -> Result<(), SerializationError> {
    let s = message.downcast_ref::<String>().ok_or(SerializationError::InvalidFormat)?;
    buf.extend_from_slice(s.as_bytes());
    Ok(())
  }

  fn from_binary_buf(&self, bytes: &[u8], _manifest: &str) -> Result<Box<dyn Any + Send + Sync>, SerializationError> {
    let s = core::str::from_utf8(bytes).map_err(|_| SerializationError::InvalidFormat)?;
    Ok(Box::new(String::from(s)))
  }
}

#[test]
fn round_trip_via_buffer() {
  let serializer = StubByteBufferSerializer;
  let message = String::from("hello");

  let mut buf: Vec<u8> = Vec::new();
  match serializer.to_binary_buf(&message as &(dyn Any + Send + Sync), &mut buf) {
    | Ok(()) => {},
    | Err(e) => panic!("to_binary_buf failed: {e:?}"),
  }

  assert_eq!(buf, b"hello");

  let restored = match serializer.from_binary_buf(&buf, "") {
    | Ok(v) => v,
    | Err(e) => panic!("from_binary_buf failed: {e:?}"),
  };
  let restored_str = match restored.downcast_ref::<String>() {
    | Some(v) => v,
    | None => panic!("downcast to String failed"),
  };
  assert_eq!(restored_str, "hello");
}

#[test]
fn appends_to_existing_buffer_content() {
  let serializer = StubByteBufferSerializer;
  let message = String::from("world");

  let mut buf: Vec<u8> = vec![0x01, 0x02];
  match serializer.to_binary_buf(&message as &(dyn Any + Send + Sync), &mut buf) {
    | Ok(()) => {},
    | Err(e) => panic!("to_binary_buf failed: {e:?}"),
  }

  // シリアライザは追記する。既存バイトは保持される。
  assert_eq!(&buf[..2], &[0x01, 0x02]);
  assert_eq!(&buf[2..], b"world");
}

#[test]
fn invalid_message_type_returns_error() {
  let serializer = StubByteBufferSerializer;
  let message: i32 = 42;

  let mut buf: Vec<u8> = Vec::new();
  let result = serializer.to_binary_buf(&message as &(dyn Any + Send + Sync), &mut buf);

  match result {
    | Err(err) => assert!(err.is_invalid_format()),
    | Ok(_) => panic!("expected InvalidFormat error"),
  }
}

#[test]
fn trait_object_is_send_sync() {
  fn assert_send_sync<T: Send + Sync>() {}
  assert_send_sync::<StubByteBufferSerializer>();
}

#[test]
fn serialize_via_trait_object() {
  // トレイトオブジェクト経由でシリアライズできることを確認する。
  let serializer: &dyn ByteBufferSerializer = &StubByteBufferSerializer;
  let mut buf: Vec<u8> = Vec::new();
  let message = String::from("test");
  let result = serializer.to_binary_buf(&message as &(dyn Any + Send + Sync), &mut buf);
  assert!(result.is_ok());
  assert_eq!(buf, b"test");
}
