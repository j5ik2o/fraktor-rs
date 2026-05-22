//! Persistence serializer registration.

use core::any::TypeId;

use fraktor_actor_core_kernel_rs::serialization::{
  SerializationError, Serializer, SerializerId, contribution::SerializationRegistryContributor,
  serialization_registry::SerializationRegistry,
};
use fraktor_utils_core_rs::sync::ArcShared;

use crate::{
  persistent::{AtomicWrite, PersistentRepr},
  serialization::{MessageSerializer, SnapshotPayload, SnapshotSerializer},
};

/// Serializer id for persistence journal messages.
pub const MESSAGE_SERIALIZER_ID: SerializerId = SerializerId::from_raw(41);

/// Serializer id for persistence snapshot payloads.
pub const SNAPSHOT_SERIALIZER_ID: SerializerId = SerializerId::from_raw(42);

/// Contributes persistence serializers to a serialization registry.
pub struct PersistenceSerializationContributor;

impl PersistenceSerializationContributor {
  /// Creates a new persistence serialization contributor.
  #[must_use]
  pub const fn new() -> Self {
    Self
  }
}

impl Default for PersistenceSerializationContributor {
  fn default() -> Self {
    Self::new()
  }
}

impl SerializationRegistryContributor for PersistenceSerializationContributor {
  fn contribute(&self, registry: &ArcShared<SerializationRegistry>) -> Result<(), SerializationError> {
    register_persistence_serializers(registry)
  }
}

/// Registers persistence serializers and type bindings.
///
/// # Errors
///
/// Returns [`SerializationError`] when an id or binding collision is detected.
pub fn register_persistence_serializers(registry: &ArcShared<SerializationRegistry>) -> Result<(), SerializationError> {
  register_serializer(
    registry,
    MESSAGE_SERIALIZER_ID,
    ArcShared::new(MessageSerializer::new(MESSAGE_SERIALIZER_ID, registry.downgrade())),
  )?;
  register_serializer(
    registry,
    SNAPSHOT_SERIALIZER_ID,
    ArcShared::new(SnapshotSerializer::new(SNAPSHOT_SERIALIZER_ID, registry.downgrade())),
  )?;
  register_binding::<PersistentRepr>(registry, "PersistentRepr", MESSAGE_SERIALIZER_ID)?;
  register_binding::<AtomicWrite>(registry, "AtomicWrite", MESSAGE_SERIALIZER_ID)?;
  register_binding::<SnapshotPayload>(registry, "SnapshotPayload", SNAPSHOT_SERIALIZER_ID)
}

fn register_serializer(
  registry: &SerializationRegistry,
  id: SerializerId,
  serializer: ArcShared<dyn Serializer>,
) -> Result<(), SerializationError> {
  if let Some(existing) = registry.registered_serializer(id) {
    if existing.as_any().type_id() == serializer.as_any().type_id() {
      return Ok(());
    }
    return Err(SerializationError::UnknownSerializer(id));
  }
  if registry.register_serializer(id, serializer) { Ok(()) } else { Err(SerializationError::UnknownSerializer(id)) }
}

fn register_binding<T: 'static>(
  registry: &SerializationRegistry,
  type_name: &'static str,
  serializer_id: SerializerId,
) -> Result<(), SerializationError> {
  let type_id = TypeId::of::<T>();
  if let Some(existing) = registry.binding_for(type_id) {
    if existing == serializer_id {
      return Ok(());
    }
    return Err(SerializationError::UnknownSerializer(existing));
  }
  registry.register_binding(type_id, type_name, serializer_id)
}
