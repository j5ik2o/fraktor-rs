//! Serializer for persistence snapshot payloads.

#[cfg(test)]
#[path = "snapshot_serializer_test.rs"]
mod tests;

use alloc::{boxed::Box, string::String, vec::Vec};
use core::{
  any::{Any, TypeId, type_name_of_val},
  ops::Deref,
};

use fraktor_actor_core_kernel_rs::serialization::{
  SerializationDelegator, SerializationError, SerializedMessage, Serializer, SerializerId,
  serialization_registry::SerializationRegistry,
};
use fraktor_utils_core_rs::sync::{ArcShared, WeakShared};

use crate::serialization::SnapshotPayload;

/// Serializes snapshot payload wrappers.
pub struct SnapshotSerializer {
  id:       SerializerId,
  registry: WeakShared<SerializationRegistry>,
}

impl SnapshotSerializer {
  /// Creates a new snapshot serializer.
  #[must_use]
  pub const fn new(id: SerializerId, registry: WeakShared<SerializationRegistry>) -> Self {
    Self { id, registry }
  }

  fn registry(&self) -> Result<ArcShared<SerializationRegistry>, SerializationError> {
    self.registry.upgrade().ok_or(SerializationError::Uninitialized)
  }

  fn has_valid_manifest(message: &SerializedMessage) -> bool {
    matches!(message.manifest(), Some(manifest) if !manifest.is_empty())
  }
}

impl Serializer for SnapshotSerializer {
  fn identifier(&self) -> SerializerId {
    self.id
  }

  fn include_manifest(&self) -> bool {
    true
  }

  fn to_binary(&self, message: &(dyn Any + Send + Sync)) -> Result<Vec<u8>, SerializationError> {
    let payload = message.downcast_ref::<SnapshotPayload>().ok_or(SerializationError::InvalidFormat)?;
    let registry = self.registry()?;
    let delegator = SerializationDelegator::new(&registry);
    let data = payload.data().deref();
    let type_name = registry.binding_name(data.type_id()).unwrap_or_else(|| String::from(type_name_of_val(data)));
    let nested = delegator.serialize(data, &type_name)?;
    if !Self::has_valid_manifest(&nested) {
      return Err(SerializationError::InvalidFormat);
    }
    Ok(nested.encode())
  }

  fn from_binary(
    &self,
    bytes: &[u8],
    _type_hint: Option<TypeId>,
  ) -> Result<Box<dyn Any + Send + Sync>, SerializationError> {
    let registry = self.registry()?;
    let delegator = SerializationDelegator::new(&registry);
    let nested = SerializedMessage::decode(bytes)?;
    if !Self::has_valid_manifest(&nested) {
      return Err(SerializationError::InvalidFormat);
    }
    let data = delegator.deserialize(&nested, None)?;
    Ok(Box::new(SnapshotPayload::new(ArcShared::from_boxed(data))))
  }

  fn as_any(&self) -> &(dyn Any + Send + Sync) {
    self
  }
}
