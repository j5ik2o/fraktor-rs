//! Serialization extension implementation.

use alloc::{
  boxed::Box,
  string::{String, ToString},
};
use core::any::Any;

use cellactor_utils_core_rs::sync::ArcShared;
use serde::{Serialize, de::DeserializeOwned};

use super::super::{
  bincode_serializer::BincodeSerializer, error::SerializationError, payload::SerializedPayload,
  registry::SerializerRegistry, serializer::SerializerHandle,
};
use crate::{RuntimeToolbox, extension::Extension};

/// Serialization extension that manages serializer registration and bindings.
pub struct Serialization<TB: RuntimeToolbox + 'static> {
  registry: ArcShared<SerializerRegistry<TB>>,
}

impl<TB: RuntimeToolbox + 'static> Extension<TB> for Serialization<TB> {}

unsafe impl<TB: RuntimeToolbox + 'static> Send for Serialization<TB> {}
unsafe impl<TB: RuntimeToolbox + 'static> Sync for Serialization<TB> {}

impl<TB: RuntimeToolbox + 'static> Serialization<TB> {
  pub(super) fn new() -> Self {
    let registry = ArcShared::new(SerializerRegistry::new());
    let handle = SerializerHandle::new(BincodeSerializer);
    if let Err(error) = registry.register_serializer(handle) {
      panic!("failed to register built-in serializer: {error}");
    }
    Self { registry }
  }

  /// Serializes the provided value into a [`SerializedPayload`].
  ///
  /// # Errors
  ///
  /// Returns an error if serialization fails or the type is not registered.
  pub fn serialize<T>(&self, value: &T) -> Result<SerializedPayload, SerializationError>
  where
    T: Serialize + Send + Sync + 'static, {
    let binding = self.registry.find_binding_by_type::<T>()?;
    let erased: &dyn erased_serde::Serialize = value;
    let bytes = binding.serializer().serialize_erased(erased)?;
    Ok(SerializedPayload::new(binding.serializer_id(), binding.manifest().to_string(), bytes))
  }

  /// Deserializes bytes into `T`, verifying the manifest along the way.
  ///
  /// # Errors
  ///
  /// Returns an error if deserialization fails or the manifest does not match.
  pub fn deserialize<T>(&self, bytes: &[u8], manifest: &str) -> Result<T, SerializationError>
  where
    T: DeserializeOwned + Send + 'static, {
    let binding = self.registry.find_binding_by_type::<T>()?;
    if binding.manifest() != manifest {
      return Err(SerializationError::TypeMismatch {
        expected: binding.manifest().to_string(),
        found:    String::from(manifest),
      });
    }
    binding.deserialize_as::<T>(bytes)
  }

  /// Returns the serializer handle bound to type `T`.
  ///
  /// # Errors
  ///
  /// Returns an error if the type is not registered.
  pub fn find_serializer_for<T>(&self) -> Result<SerializerHandle, SerializationError>
  where
    T: Any + 'static, {
    let binding = self.registry.find_binding_by_type::<T>()?;
    Ok(binding.serializer().clone())
  }

  /// Returns the registry for custom bindings.
  #[must_use]
  pub fn registry(&self) -> ArcShared<SerializerRegistry<TB>> {
    self.registry.clone()
  }

  /// Deserializes an opaque payload into a boxed object.
  ///
  /// # Errors
  ///
  /// Returns an error if deserialization fails or the serializer is not found.
  pub fn deserialize_payload(&self, payload: &SerializedPayload) -> Result<Box<dyn Any + Send>, SerializationError> {
    let serializer = self.registry.find_serializer_by_id(payload.serializer_id())?;
    let binding = self.registry.find_binding_by_manifest(payload.serializer_id(), payload.manifest())?;
    if binding.serializer_id() != serializer.identifier() {
      return Err(SerializationError::SerializerNotFound(payload.serializer_id()));
    }
    binding.deserialize_boxed(payload.bytes().as_ref())
  }
}
