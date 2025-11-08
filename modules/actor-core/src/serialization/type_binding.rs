//! Type binding for serialization.

use alloc::{
  boxed::Box,
  string::{String, ToString},
};
use core::any::{Any, TypeId};

use cellactor_utils_core_rs::sync::ArcShared;

use super::{error::SerializationError, serializer::SerializerHandle};

/// Shared type alias for boxed deserializers.
pub(super) type BoxedDeserializer =
  ArcShared<dyn Fn(&[u8]) -> Result<Box<dyn Any + Send>, SerializationError> + Send + Sync>;

/// Immutable metadata and typed deserializers for a registered type.
pub(super) struct TypeBinding {
  #[allow(dead_code)]
  type_id:           TypeId,
  manifest:          String,
  serializer_id:     u32,
  serializer:        SerializerHandle,
  deserialize_boxed: BoxedDeserializer,
}

impl TypeBinding {
  pub(super) fn new<T, F>(
    type_id: TypeId,
    manifest: String,
    serializer_id: u32,
    serializer: &SerializerHandle,
    deserializer: F,
  ) -> Self
  where
    T: Any + Send + Sync + 'static,
    F: Fn(&[u8]) -> Result<T, SerializationError> + Send + Sync + 'static, {
    let deserialize_boxed: BoxedDeserializer =
      ArcShared::new(move |bytes| deserializer(bytes).map(|value| Box::new(value) as Box<dyn Any + Send>));
    Self { type_id, manifest, serializer_id, serializer: serializer.clone(), deserialize_boxed }
  }

  /// Returns the stored manifest string.
  #[must_use]
  pub(super) fn manifest(&self) -> &str {
    &self.manifest
  }

  /// Returns the serializer identifier.
  #[must_use]
  pub(super) const fn serializer_id(&self) -> u32 {
    self.serializer_id
  }

  /// Returns the registered [`TypeId`].
  #[must_use]
  #[allow(dead_code)]
  pub(super) const fn type_id(&self) -> TypeId {
    self.type_id
  }

  /// Returns the serializer handle.
  #[must_use]
  pub(super) const fn serializer(&self) -> &SerializerHandle {
    &self.serializer
  }

  /// Runs the boxed deserializer and returns a type-erased payload.
  pub(super) fn deserialize_boxed(&self, bytes: &[u8]) -> Result<Box<dyn Any + Send>, SerializationError> {
    (self.deserialize_boxed)(bytes)
  }

  /// Runs the boxed deserializer and downcasts it to the requested type.
  pub(super) fn deserialize_as<T>(&self, bytes: &[u8]) -> Result<T, SerializationError>
  where
    T: Any + Send + 'static, {
    match self.deserialize_boxed(bytes)?.downcast::<T>() {
      | Ok(value) => Ok(*value),
      | Err(_) => Err(SerializationError::TypeMismatch {
        expected: self.manifest.clone(),
        found:    core::any::type_name::<T>().to_string(),
      }),
    }
  }
}
