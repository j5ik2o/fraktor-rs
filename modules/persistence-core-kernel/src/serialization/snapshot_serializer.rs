//! Serializer for persistence snapshot payloads.

#[cfg(test)]
#[path = "snapshot_serializer_test.rs"]
mod tests;

use alloc::{boxed::Box, string::String, vec::Vec};
use core::{
  any::{Any, TypeId},
  ops::Deref,
};

use fraktor_actor_core_kernel_rs::serialization::{
  SerializationDelegator, SerializationError, SerializedMessage, Serializer, SerializerId,
  serialization_registry::SerializationRegistry,
};
use fraktor_utils_core_rs::sync::{ArcShared, WeakShared};

use crate::serialization::{SnapshotPayload, wire};

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

  pub(crate) fn uses_registry(&self, registry: &ArcShared<SerializationRegistry>) -> bool {
    self.registry.upgrade().is_some_and(|registered| ArcShared::ptr_eq(&registered, registry))
  }

  fn registry(&self) -> Result<ArcShared<SerializationRegistry>, SerializationError> {
    self.registry.upgrade().ok_or(SerializationError::Uninitialized)
  }

  fn has_valid_manifest(message: &SerializedMessage) -> bool {
    !matches!(message.manifest(), Some(manifest) if manifest.is_empty())
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
    let type_name = registry.binding_name(data.type_id()).unwrap_or_else(|| String::from("SnapshotPayload data"));
    let nested = delegator.serialize(data, &type_name)?;
    if !Self::has_valid_manifest(&nested) {
      return Err(SerializationError::InvalidFormat);
    }
    let mut buffer = Vec::new();
    wire::write_string(&mut buffer, &type_name)?;
    wire::write_serialized(&mut buffer, &nested)?;
    Ok(buffer)
  }

  fn from_binary(
    &self,
    bytes: &[u8],
    _type_hint: Option<TypeId>,
  ) -> Result<Box<dyn Any + Send + Sync>, SerializationError> {
    let registry = self.registry()?;
    let delegator = SerializationDelegator::new(&registry);
    let mut cursor = 0;
    let type_name = wire::read_string(bytes, &mut cursor)?;
    let type_hint = registry.type_id_for_binding_name(&type_name);
    let nested = wire::read_serialized(bytes, &mut cursor)?;
    wire::ensure_finished(bytes, cursor)?;
    if !Self::has_valid_manifest(&nested) {
      return Err(SerializationError::InvalidFormat);
    }
    let data = delegator.deserialize(&nested, type_hint)?;
    Ok(Box::new(SnapshotPayload::new(ArcShared::from_boxed(data))))
  }

  fn as_any(&self) -> &(dyn Any + Send + Sync) {
    self
  }
}
