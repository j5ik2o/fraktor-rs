//! Serializer handle.

use alloc::boxed::Box;
use core::any::Any;

use cellactor_utils_core_rs::sync::ArcShared;
use erased_serde::Serialize as ErasedSerialize;

use super::{
  super::{bytes::Bytes, error::SerializationError},
  r#impl::SerializerImpl,
};

/// Cloneable handle that hides the underlying serializer implementation.
#[derive(Clone)]
pub struct SerializerHandle {
  inner: ArcShared<dyn SerializerImpl>,
}

impl SerializerHandle {
  /// Creates a new handle from the provided serializer implementation.
  #[must_use]
  pub fn new<T>(serializer: T) -> Self
  where
    T: SerializerImpl + 'static, {
    Self { inner: ArcShared::new(serializer) }
  }

  /// Returns the numeric identifier associated with the serializer.
  #[must_use]
  pub fn identifier(&self) -> u32 {
    self.inner.identifier()
  }

  /// Serializes an erased [`serde::Serialize`] value.
  ///
  /// # Errors
  ///
  /// Returns an error if serialization fails.
  pub fn serialize_erased(&self, value: &dyn ErasedSerialize) -> Result<Bytes, SerializationError> {
    self.inner.serialize_erased(value)
  }

  /// Deserializes bytes into a boxed [`Any`] value.
  ///
  /// # Errors
  ///
  /// Returns an error if deserialization fails.
  pub fn deserialize(&self, bytes: &[u8], manifest: &str) -> Result<Box<dyn Any + Send>, SerializationError> {
    self.inner.deserialize(bytes, manifest)
  }
}
