//! Serializer for persistent journal records.

#[cfg(test)]
#[path = "message_serializer_test.rs"]
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

use crate::{
  persistent::{AtomicWrite, PersistentRepr},
  serialization::wire::{self, ATOMIC_WRITE_TAG, PERSISTENT_REPR_TAG},
};

/// Serializes [`PersistentRepr`] and [`AtomicWrite`] records.
pub struct MessageSerializer {
  id:       SerializerId,
  registry: WeakShared<SerializationRegistry>,
}

impl MessageSerializer {
  /// Creates a new message serializer.
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

  fn encode_repr(&self, repr: &PersistentRepr) -> Result<Vec<u8>, SerializationError> {
    let registry = self.registry()?;
    let delegator = SerializationDelegator::new(&registry);
    let mut buffer = Vec::new();
    wire::write_string(&mut buffer, repr.persistence_id())?;
    wire::write_u64(&mut buffer, repr.sequence_nr());
    let payload = repr.payload().deref();
    let payload_type_id = payload.type_id();
    let payload_binding_name = registry.binding_name(payload_type_id);
    let payload_type_name = payload_binding_name.as_deref().unwrap_or("");
    let payload_serializer_hint = payload_binding_name.as_deref().unwrap_or("PersistentRepr payload");
    let serialized_payload =
      Self::serialize_nested(&delegator, payload, payload_serializer_hint, payload_binding_name.is_some())?;
    wire::write_string(&mut buffer, payload_type_name)?;
    wire::write_serialized(&mut buffer, &serialized_payload)?;
    wire::write_string(&mut buffer, repr.manifest())?;
    wire::write_string(&mut buffer, repr.writer_uuid())?;
    wire::write_u64(&mut buffer, repr.timestamp());
    wire::write_bool(&mut buffer, repr.deleted());
    let adapter_type_id = repr.adapter_type_id();
    let adapter_type_name = if adapter_type_id == payload_type_id {
      // Empty adapter key is a wire-level shorthand for "use the decoded payload type".
      String::new()
    } else {
      registry.binding_name(adapter_type_id).ok_or(SerializationError::InvalidFormat)?
    };
    wire::write_string(&mut buffer, &adapter_type_name)?;
    if let Some(metadata) = repr.metadata() {
      wire::write_bool(&mut buffer, true);
      let metadata = metadata.deref();
      let metadata_binding_name = registry.binding_name(metadata.type_id());
      let metadata_type_name = metadata_binding_name.as_deref().unwrap_or("");
      let metadata_serializer_hint = metadata_binding_name.as_deref().unwrap_or("PersistentRepr metadata");
      let serialized_metadata =
        Self::serialize_nested(&delegator, metadata, metadata_serializer_hint, metadata_binding_name.is_some())?;
      wire::write_string(&mut buffer, metadata_type_name)?;
      wire::write_serialized(&mut buffer, &serialized_metadata)?;
    } else {
      wire::write_bool(&mut buffer, false);
    }
    Ok(buffer)
  }

  fn decode_repr(&self, bytes: &[u8]) -> Result<PersistentRepr, SerializationError> {
    let registry = self.registry()?;
    let delegator = SerializationDelegator::new(&registry);
    let mut cursor = 0;
    let persistence_id = wire::read_string(bytes, &mut cursor)?;
    let sequence_nr = wire::read_u64(bytes, &mut cursor)?;
    let payload_type_name = wire::read_string(bytes, &mut cursor)?;
    let payload_type_hint = Self::type_hint_for_wire_name(&registry, &payload_type_name);
    let payload = Self::deserialize_nested(&delegator, &wire::read_serialized(bytes, &mut cursor)?, payload_type_hint)?;
    let manifest = wire::read_string(bytes, &mut cursor)?;
    let writer_uuid = wire::read_string(bytes, &mut cursor)?;
    let timestamp = wire::read_u64(bytes, &mut cursor)?;
    let deleted = wire::read_bool(bytes, &mut cursor)?;
    let adapter_type_name = wire::read_string(bytes, &mut cursor)?;
    let has_metadata = wire::read_bool(bytes, &mut cursor)?;
    let mut repr = PersistentRepr::new(persistence_id, sequence_nr, ArcShared::from_boxed(payload))
      .with_manifest(manifest)
      .with_writer_uuid(writer_uuid)
      .with_timestamp(timestamp)
      .with_deleted(deleted);
    if !adapter_type_name.is_empty() {
      let adapter_type_id =
        registry.type_id_for_binding_name(&adapter_type_name).ok_or(SerializationError::InvalidFormat)?;
      repr = repr.with_adapter_type_id(adapter_type_id);
    }
    if has_metadata {
      let metadata_type_name = wire::read_string(bytes, &mut cursor)?;
      let metadata_type_hint = Self::type_hint_for_wire_name(&registry, &metadata_type_name);
      let metadata =
        Self::deserialize_nested(&delegator, &wire::read_serialized(bytes, &mut cursor)?, metadata_type_hint)?;
      repr = repr.with_metadata(ArcShared::from_boxed(metadata));
    }
    wire::ensure_finished(bytes, cursor)?;
    Ok(repr)
  }

  fn serialize_nested(
    delegator: &SerializationDelegator<'_>,
    message: &(dyn Any + Send + Sync),
    type_name: &str,
    has_binding_name: bool,
  ) -> Result<SerializedMessage, SerializationError> {
    let nested = delegator.serialize(message, type_name)?;
    if !Self::has_valid_manifest(&nested) {
      return Err(SerializationError::InvalidFormat);
    }
    if !has_binding_name && nested.manifest().is_none() {
      return Err(SerializationError::InvalidFormat);
    }
    Ok(nested)
  }

  fn has_valid_manifest(message: &SerializedMessage) -> bool {
    match message.manifest() {
      | Some(manifest) => !manifest.is_empty(),
      | None => true,
    }
  }

  fn type_hint_for_wire_name(registry: &SerializationRegistry, type_name: &str) -> Option<TypeId> {
    if type_name.is_empty() { None } else { registry.type_id_for_binding_name(type_name) }
  }

  fn deserialize_nested(
    delegator: &SerializationDelegator<'_>,
    message: &SerializedMessage,
    type_hint: Option<TypeId>,
  ) -> Result<Box<dyn Any + Send + Sync>, SerializationError> {
    if !Self::has_valid_manifest(message) {
      return Err(SerializationError::InvalidFormat);
    }
    delegator.deserialize(message, type_hint)
  }
}

impl Serializer for MessageSerializer {
  fn identifier(&self) -> SerializerId {
    self.id
  }

  fn include_manifest(&self) -> bool {
    true
  }

  fn to_binary(&self, message: &(dyn Any + Send + Sync)) -> Result<Vec<u8>, SerializationError> {
    let mut buffer = Vec::new();
    if let Some(repr) = message.downcast_ref::<PersistentRepr>() {
      wire::write_u8(&mut buffer, PERSISTENT_REPR_TAG);
      wire::write_bytes(&mut buffer, &self.encode_repr(repr)?)?;
      return Ok(buffer);
    }
    if let Some(atomic_write) = message.downcast_ref::<AtomicWrite>() {
      wire::write_u8(&mut buffer, ATOMIC_WRITE_TAG);
      let count = u32::try_from(atomic_write.size()).map_err(|_| SerializationError::InvalidFormat)?;
      wire::write_u32(&mut buffer, count);
      for repr in atomic_write.payload() {
        wire::write_bytes(&mut buffer, &self.encode_repr(repr)?)?;
      }
      return Ok(buffer);
    }
    Err(SerializationError::InvalidFormat)
  }

  fn from_binary(
    &self,
    bytes: &[u8],
    _type_hint: Option<TypeId>,
  ) -> Result<Box<dyn Any + Send + Sync>, SerializationError> {
    let mut cursor = 0;
    match wire::read_u8(bytes, &mut cursor)? {
      | PERSISTENT_REPR_TAG => {
        let repr = self.decode_repr(wire::read_bytes(bytes, &mut cursor)?)?;
        wire::ensure_finished(bytes, cursor)?;
        Ok(Box::new(repr))
      },
      | ATOMIC_WRITE_TAG => {
        let count = wire::read_u32(bytes, &mut cursor)?;
        let mut payload = Vec::new();
        for _ in 0..count {
          payload.push(self.decode_repr(wire::read_bytes(bytes, &mut cursor)?)?);
        }
        wire::ensure_finished(bytes, cursor)?;
        let atomic_write = AtomicWrite::new(payload).map_err(|_| SerializationError::InvalidFormat)?;
        Ok(Box::new(atomic_write))
      },
      | _ => Err(SerializationError::InvalidFormat),
    }
  }

  fn as_any(&self) -> &(dyn Any + Send + Sync) {
    self
  }
}
