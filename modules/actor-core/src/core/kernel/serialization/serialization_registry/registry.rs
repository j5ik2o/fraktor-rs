//! Serialization registry implementation.

use alloc::{string::String, vec::Vec};
use core::any::TypeId;

use ahash::RandomState;
use fraktor_utils_core_rs::core::sync::{ArcShared, DefaultRwLock, SharedRwLock};
use hashbrown::{HashMap, hash_map::Entry};

use super::SerializerResolutionOrigin;
use crate::core::kernel::serialization::{
  NotSerializableError, SerializationError, SerializationSetup, Serializer, SerializerId, TransportInformation,
};

/// Registry that resolves serializers based on type identifiers.
///
/// `remote_manifests` is populated once in [`Self::from_setup`] from the immutable
/// [`SerializationSetup`] and never mutated afterwards, so it does not need a lock.
#[allow(clippy::type_complexity)]
pub struct SerializationRegistry {
  serializers:      SharedRwLock<HashMap<SerializerId, ArcShared<dyn Serializer>, RandomState>>,
  bindings:         SharedRwLock<HashMap<TypeId, SerializerId, RandomState>>,
  binding_names:    SharedRwLock<HashMap<TypeId, String, RandomState>>,
  remote_manifests: HashMap<TypeId, String, RandomState>,
  manifest_routes:  SharedRwLock<HashMap<String, Vec<(u8, SerializerId)>, RandomState>>,
  cache:            SharedRwLock<HashMap<TypeId, SerializerId, RandomState>>,
  fallback:         SerializerId,
}

impl SerializationRegistry {
  /// Creates a registry from the provided setup.
  #[must_use]
  pub fn from_setup(setup: &SerializationSetup) -> Self {
    let mut serializers = HashMap::with_hasher(RandomState::new());
    serializers.extend(setup.serializers_ref().iter().map(|(id, serializer)| (*id, serializer.clone())));
    let mut bindings = HashMap::with_hasher(RandomState::new());
    bindings.extend(setup.bindings_ref().iter().map(|(ty, id)| (*ty, *id)));
    let mut binding_names = HashMap::with_hasher(RandomState::new());
    binding_names.extend(setup.binding_names_ref().iter().map(|(ty, name)| (*ty, name.clone())));
    let mut remote_manifests = HashMap::with_hasher(RandomState::new());
    remote_manifests.extend(setup.remote_manifests_ref().iter().map(|(ty, manifest)| (*ty, manifest.clone())));
    let mut manifest_routes = HashMap::with_hasher(RandomState::new());
    manifest_routes
      .extend(setup.manifest_routes_ref().iter().map(|(manifest, routes)| (manifest.clone(), routes.clone())));
    Self {
      serializers: SharedRwLock::new_with_driver::<DefaultRwLock<_>>(serializers),
      bindings: SharedRwLock::new_with_driver::<DefaultRwLock<_>>(bindings),
      binding_names: SharedRwLock::new_with_driver::<DefaultRwLock<_>>(binding_names),
      remote_manifests,
      manifest_routes: SharedRwLock::new_with_driver::<DefaultRwLock<_>>(manifest_routes),
      cache: SharedRwLock::new_with_driver::<DefaultRwLock<_>>(HashMap::with_hasher(RandomState::new())),
      fallback: setup.fallback_serializer(),
    }
  }

  fn serializer_by_id_raw(&self, id: SerializerId) -> Option<ArcShared<dyn Serializer>> {
    self.serializers.with_read(|serializers| serializers.get(&id).cloned())
  }

  fn not_serializable(
    type_name: &str,
    serializer_id: Option<SerializerId>,
    transport_hint: Option<TransportInformation>,
  ) -> SerializationError {
    SerializationError::NotSerializable(NotSerializableError::new(type_name, serializer_id, None, None, transport_hint))
  }

  fn cache_insert(&self, type_id: TypeId, serializer_id: SerializerId) {
    self.cache.with_write(|cache| {
      cache.insert(type_id, serializer_id);
    });
  }

  fn cache_remove(&self, type_id: TypeId) {
    self.cache.with_write(|cache| {
      cache.remove(&type_id);
    });
  }

  /// Returns the serializer registered for the type, performing fallback resolution if required.
  ///
  /// # Errors
  ///
  /// Returns `SerializationError::NotSerializable` if no serializer is registered for the resolved
  /// ID.
  pub fn serializer_for_type(
    &self,
    type_id: TypeId,
    type_name: &str,
    transport_hint: Option<TransportInformation>,
  ) -> Result<(ArcShared<dyn Serializer>, SerializerResolutionOrigin), SerializationError> {
    if let Some(existing) = self.cache.with_read(|cache| cache.get(&type_id).copied()) {
      if let Some(serializer) = self.serializer_by_id_raw(existing) {
        return Ok((serializer, SerializerResolutionOrigin::Cache));
      }
      self.cache_remove(type_id);
    }

    let (resolved, origin) = if let Some(bound) = self.bindings.with_read(|bindings| bindings.get(&type_id).copied()) {
      (bound, SerializerResolutionOrigin::Binding)
    } else {
      (self.fallback, SerializerResolutionOrigin::Fallback)
    };

    if let Some(serializer) = self.serializer_by_id_raw(resolved) {
      self.cache_insert(type_id, resolved);
      return Ok((serializer, origin));
    }
    self.cache_remove(type_id);
    Err(Self::not_serializable(type_name, Some(resolved), transport_hint))
  }

  /// Returns the serializer identified by id.
  ///
  /// # Errors
  ///
  /// Returns `SerializationError::UnknownSerializer` if no serializer is registered with the given
  /// ID.
  pub fn serializer_by_id(&self, id: SerializerId) -> Result<ArcShared<dyn Serializer>, SerializationError> {
    self.serializer_by_id_raw(id).ok_or(SerializationError::UnknownSerializer(id))
  }

  /// Inserts a serializer instance if absent.
  #[must_use]
  pub fn register_serializer(&self, id: SerializerId, serializer: ArcShared<dyn Serializer>) -> bool {
    self.serializers.with_write(|guard| match guard.entry(id) {
      | Entry::Occupied(_) => false,
      | Entry::Vacant(slot) => {
        slot.insert(serializer);
        true
      },
    })
  }

  /// Registers a binding at runtime (used by adapters/extensions).
  ///
  /// # Errors
  ///
  /// Returns `SerializationError::UnknownSerializer` if the specified serializer ID is not
  /// registered.
  pub fn register_binding(
    &self,
    type_id: TypeId,
    type_name: impl Into<String>,
    serializer_id: SerializerId,
  ) -> Result<(), SerializationError> {
    if self.serializer_by_id_raw(serializer_id).is_none() {
      return Err(SerializationError::UnknownSerializer(serializer_id));
    }
    self.bindings.with_write(|bindings| {
      bindings.insert(type_id, serializer_id);
    });
    self.binding_names.with_write(|binding_names| {
      binding_names.insert(type_id, type_name.into());
    });
    self.cache_remove(type_id);
    Ok(())
  }

  /// Returns the serializers registered for the specified manifest in priority order.
  #[must_use]
  pub fn serializers_for_manifest(&self, manifest: &str) -> Vec<ArcShared<dyn Serializer>> {
    self
      .manifest_routes
      .with_read(|routes| {
        routes.get(manifest).map(|entries| {
          entries.iter().filter_map(|(_, serializer_id)| self.serializer_by_id_raw(*serializer_id)).collect::<Vec<_>>()
        })
      })
      .unwrap_or_default()
  }

  /// Returns the recorded binding name for the provided type identifier.
  #[must_use]
  pub fn binding_name(&self, type_id: TypeId) -> Option<String> {
    self.binding_names.with_read(|binding_names| binding_names.get(&type_id).cloned())
  }

  /// Returns the remote manifest registered for the provided type identifier.
  #[must_use]
  pub fn manifest_for(&self, type_id: TypeId) -> Option<&str> {
    self.remote_manifests.get(&type_id).map(String::as_str)
  }

  /// Clears cached lookups (used during shutdown).
  pub fn clear_cache(&self) {
    self.cache.with_write(|cache| cache.clear());
  }
}
