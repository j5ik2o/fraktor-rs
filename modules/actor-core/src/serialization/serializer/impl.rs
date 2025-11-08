//! Serializer implementation trait.

use alloc::boxed::Box;
use core::any::Any;

use erased_serde::Serialize as ErasedSerialize;

use super::super::{bytes::Bytes, error::SerializationError};

/// Object-safe trait that low-level serializers must implement.
pub trait SerializerImpl: Send + Sync {
  /// Unique identifier that must remain stable across versions.
  fn identifier(&self) -> u32;

  /// Serializes the provided value into a byte buffer.
  ///
  /// # Errors
  ///
  /// Returns an error if serialization fails.
  fn serialize_erased(&self, value: &dyn ErasedSerialize) -> Result<Bytes, SerializationError>;

  /// Deserializes bytes into a boxed value based on the manifest.
  ///
  /// # Errors
  ///
  /// Returns an error if deserialization fails.
  fn deserialize(&self, bytes: &[u8], manifest: &str) -> Result<Box<dyn Any + Send>, SerializationError>;
}
