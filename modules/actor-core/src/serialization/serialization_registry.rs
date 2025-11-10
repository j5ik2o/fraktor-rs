//! Runtime serialization registry responsible for resolving serializers by type.

#[cfg(test)]
mod tests;

use alloc::{string::String, vec::Vec};
use core::any::TypeId;

use cellactor_utils_core_rs::{
  runtime_toolbox::SyncMutexFamily,
  sync::{sync_mutex_like::SyncMutexLike, ArcShared, NoStdToolbox},
};
use hashbrown::HashMap;

use crate::{RuntimeToolbox, ToolboxMutex};

use super::{
  error::SerializationError, not_serializable_error::NotSerializableError, serialization_setup::SerializationSetup,
  serializer::Serializer, serializer_id::SerializerId, transport_information::TransportInformation,
};

/// Registry that resolves serializers based on type identifiers.
pub struct SerializationRegistryGeneric<TB: RuntimeToolbox> {
  serializers:     ToolboxMutex<HashMap<SerializerId, ArcShared<dyn Serializer>>, TB>,
  bindings:        ToolboxMutex<HashMap<TypeId, SerializerId>, TB>,
  binding_names:   ToolboxMutex<HashMap<TypeId, String>, TB>,
  manifest_routes: ToolboxMutex<HashMap<String, Vec<(u8, SerializerId)>>, TB>,
  cache:           ToolboxMutex<HashMap<TypeId, SerializerId>, TB>,
  fallback:        SerializerId,
}

impl<TB: RuntimeToolbox> SerializationRegistryGeneric<TB> {
  /// Creates a registry from the provided setup.
  #[must_use]
  pub fn from_setup(setup: &SerializationSetup) -> Self {
    let serializers = setup
      .serializers_ref()
      .iter()
      .map(|(id, serializer)| (*id, serializer.clone()))
      .collect::<HashMap<_, _>>();
    let bindings = setup.bindings_ref().iter().map(|(ty, id)| (*ty, *id)).collect::<HashMap<_, _>>();
    let binding_names = setup
      .binding_names_ref()
      .iter()
      .map(|(ty, name)| (*ty, name.clone()))
      .collect::<HashMap<_, _>>();
    let manifest_routes = setup
      .manifest_routes_ref()
      .iter()
      .map(|(manifest, routes)| (manifest.clone(), routes.clone()))
      .collect::<HashMap<_, _>>();
    Self {
      serializers: <TB::MutexFamily as SyncMutexFamily>::create(serializers),
      bindings: <TB::MutexFamily as SyncMutexFamily>::create(bindings),
      binding_names: <TB::MutexFamily as SyncMutexFamily>::create(binding_names),
      manifest_routes: <TB::MutexFamily as SyncMutexFamily>::create(manifest_routes),
      cache: <TB::MutexFamily as SyncMutexFamily>::create(HashMap::new()),
      fallback: setup.fallback_serializer(),
    }
  }

  fn serializer_by_id_raw(&self, id: SerializerId) -> Option<ArcShared<dyn Serializer>> {
    self.serializers.lock().get(&id).cloned()
  }

  fn not_serializable(
    &self,
    type_name: &str,
    serializer_id: Option<SerializerId>,
    transport_hint: Option<TransportInformation>,
  ) -> SerializationError {
    SerializationError::NotSerializable(NotSerializableError::new(type_name, serializer_id, None, transport_hint))
  }

  fn cache_insert(&self, type_id: TypeId, serializer_id: SerializerId) {
    self.cache.lock().insert(type_id, serializer_id);
  }

  fn cache_remove(&self, type_id: TypeId) {
    self.cache.lock().remove(&type_id);
  }

  /// Returns the serializer registered for the type, performing fallback resolution if required.
  pub fn serializer_for_type(
    &self,
    type_id: TypeId,
    type_name: &str,
    transport_hint: Option<TransportInformation>,
  ) -> Result<ArcShared<dyn Serializer>, SerializationError> {
    if let Some(existing) = self.cache.lock().get(&type_id).copied() {
      if let Some(serializer) = self.serializer_by_id_raw(existing) {
        return Ok(serializer);
      }
      self.cache_remove(type_id);
    }

    let resolved = self.bindings.lock().get(&type_id).copied().unwrap_or(self.fallback);
    if let Some(serializer) = self.serializer_by_id_raw(resolved) {
      self.cache_insert(type_id, resolved);
      return Ok(serializer);
    }
    self.cache_remove(type_id);
    Err(self.not_serializable(type_name, Some(resolved), transport_hint))
  }

  /// Returns the serializer identified by id.
  pub fn serializer_by_id(&self, id: SerializerId) -> Result<ArcShared<dyn Serializer>, SerializationError> {
    self.serializer_by_id_raw(id).ok_or(SerializationError::UnknownSerializer(id))
  }

  /// Registers a binding at runtime (used by adapters/extensions).
  pub fn register_binding(
    &self,
    type_id: TypeId,
    type_name: impl Into<String>,
    serializer_id: SerializerId,
  ) -> Result<(), SerializationError> {
    if self.serializer_by_id_raw(serializer_id).is_none() {
      return Err(SerializationError::UnknownSerializer(serializer_id));
    }
    self.bindings.lock().insert(type_id, serializer_id);
    self.binding_names.lock().insert(type_id, type_name.into());
    self.cache_remove(type_id);
    Ok(())
  }

  /// Returns the serializers registered for the specified manifest in priority order.
  #[must_use]
  pub fn serializers_for_manifest(&self, manifest: &str) -> Vec<ArcShared<dyn Serializer>> {
    let routes = self.manifest_routes.lock();
    routes
      .get(manifest)
      .map(|entries| {
        entries
          .iter()
          .filter_map(|(_, serializer_id)| self.serializer_by_id_raw(*serializer_id))
          .collect::<Vec<_>>()
      })
      .unwrap_or_else(Vec::new)
  }

  /// Returns the recorded binding name for the provided type identifier.
  #[must_use]
  pub fn binding_name(&self, type_id: TypeId) -> Option<String> {
    self.binding_names.lock().get(&type_id).cloned()
  }
}

/// Type alias for the no_std default registry.
pub type SerializationRegistry = SerializationRegistryGeneric<NoStdToolbox>;
