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
pub const MESSAGE_SERIALIZER_ID: SerializerId = SerializerId::from_raw(43);

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
/// Re-registering the same persistence serializers is idempotent. Occupied ids or bindings that
/// point to different serializers are reported as collision errors so actor-system setup can fail
/// fast instead of silently replacing user configuration.
///
/// # Errors
///
/// Returns [`SerializationError`] when an id or binding collision is detected.
pub fn register_persistence_serializers(registry: &ArcShared<SerializationRegistry>) -> Result<(), SerializationError> {
  validate_persistence_registration(registry)?;
  register_message_serializer(registry)?;
  register_snapshot_serializer(registry)?;
  register_binding::<PersistentRepr>(registry, "PersistentRepr", MESSAGE_SERIALIZER_ID)?;
  register_binding::<AtomicWrite>(registry, "AtomicWrite", MESSAGE_SERIALIZER_ID)?;
  register_binding::<SnapshotPayload>(registry, "SnapshotPayload", SNAPSHOT_SERIALIZER_ID)
}

fn validate_persistence_registration(registry: &ArcShared<SerializationRegistry>) -> Result<(), SerializationError> {
  validate_serializer(registry, MESSAGE_SERIALIZER_ID, |existing| {
    existing.as_any().downcast_ref::<MessageSerializer>().is_some_and(|serializer| serializer.uses_registry(registry))
  })?;
  validate_serializer(registry, SNAPSHOT_SERIALIZER_ID, |existing| {
    existing.as_any().downcast_ref::<SnapshotSerializer>().is_some_and(|serializer| serializer.uses_registry(registry))
  })?;
  validate_binding::<PersistentRepr>(registry, "PersistentRepr", MESSAGE_SERIALIZER_ID)?;
  validate_binding::<AtomicWrite>(registry, "AtomicWrite", MESSAGE_SERIALIZER_ID)?;
  validate_binding::<SnapshotPayload>(registry, "SnapshotPayload", SNAPSHOT_SERIALIZER_ID)
}

fn validate_serializer<F>(
  registry: &SerializationRegistry,
  id: SerializerId,
  is_same_registration: F,
) -> Result<(), SerializationError>
where
  F: FnOnce(&dyn Serializer) -> bool, {
  if let Some(existing) = registry.registered_serializer(id) {
    if existing.identifier() == id && is_same_registration(&*existing) {
      return Ok(());
    }
    return Err(SerializationError::SerializerIdCollision(id));
  }
  Ok(())
}

fn register_message_serializer(registry: &ArcShared<SerializationRegistry>) -> Result<(), SerializationError> {
  let serializer: ArcShared<dyn Serializer> =
    ArcShared::new(MessageSerializer::new(MESSAGE_SERIALIZER_ID, registry.downgrade()));
  register_serializer(registry, MESSAGE_SERIALIZER_ID, serializer, |existing| {
    existing.as_any().downcast_ref::<MessageSerializer>().is_some_and(|serializer| serializer.uses_registry(registry))
  })
}

fn register_snapshot_serializer(registry: &ArcShared<SerializationRegistry>) -> Result<(), SerializationError> {
  let serializer: ArcShared<dyn Serializer> =
    ArcShared::new(SnapshotSerializer::new(SNAPSHOT_SERIALIZER_ID, registry.downgrade()));
  register_serializer(registry, SNAPSHOT_SERIALIZER_ID, serializer, |existing| {
    existing.as_any().downcast_ref::<SnapshotSerializer>().is_some_and(|serializer| serializer.uses_registry(registry))
  })
}

fn register_serializer<F>(
  registry: &SerializationRegistry,
  id: SerializerId,
  serializer: ArcShared<dyn Serializer>,
  is_same_registration: F,
) -> Result<(), SerializationError>
where
  F: FnOnce(&dyn Serializer) -> bool, {
  if let Some(existing) = registry.registered_serializer(id) {
    if is_same_registration(&*existing) {
      return Ok(());
    }
    return Err(SerializationError::SerializerIdCollision(id));
  }
  if registry.register_serializer(id, serializer) { Ok(()) } else { Err(SerializationError::SerializerIdCollision(id)) }
}

fn validate_binding<T: 'static>(
  registry: &SerializationRegistry,
  type_name: &'static str,
  serializer_id: SerializerId,
) -> Result<(), SerializationError> {
  let type_id = TypeId::of::<T>();
  if let Some(existing) = registry.binding_for(type_id) {
    if existing == serializer_id {
      return Ok(());
    }
    return Err(SerializationError::serializer_binding_collision(type_name, existing, serializer_id));
  }
  if let Some(existing_type_id) = registry.type_id_for_binding_name(type_name)
    && existing_type_id != type_id
  {
    let existing = registry.binding_for(existing_type_id).ok_or(SerializationError::InvalidFormat)?;
    return Err(SerializationError::serializer_binding_collision(type_name, existing, serializer_id));
  }
  Ok(())
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
    return Err(SerializationError::serializer_binding_collision(type_name, existing, serializer_id));
  }
  registry.register_binding(type_id, type_name, serializer_id)
}
