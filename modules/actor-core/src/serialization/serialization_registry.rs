//! Runtime serialization registry responsible for resolving serializers by type.

#[cfg(test)]
mod tests;

use core::any::TypeId;

use cellactor_utils_core_rs::{
  runtime_toolbox::SyncMutexFamily,
  sync::{sync_mutex_like::SyncMutexLike, ArcShared, NoStdToolbox},
};
use hashbrown::HashMap;

use crate::{RuntimeToolbox, ToolboxMutex};

use super::{serialization_setup::SerializationSetup, serializer::Serializer, serializer_id::SerializerId};

/// Registry that resolves serializers based on type identifiers.
pub struct SerializationRegistryGeneric<TB: RuntimeToolbox> {
  serializers: ToolboxMutex<HashMap<SerializerId, ArcShared<dyn Serializer>>, TB>,
  bindings:    ToolboxMutex<HashMap<TypeId, SerializerId>, TB>,
  cache:       ToolboxMutex<HashMap<TypeId, SerializerId>, TB>,
  fallback:    SerializerId,
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
    Self {
      serializers: <TB::MutexFamily as SyncMutexFamily>::create(serializers),
      bindings: <TB::MutexFamily as SyncMutexFamily>::create(bindings),
      cache: <TB::MutexFamily as SyncMutexFamily>::create(HashMap::new()),
      fallback: setup.fallback_serializer(),
    }
  }

  /// Returns the serializer registered for the type.
  #[must_use]
  pub fn serializer_for_type(&self, type_id: TypeId) -> Option<ArcShared<dyn Serializer>> {
    if let Some(existing) = self.cache.lock().get(&type_id).copied() {
      return self.serializer_by_id(existing);
    }
    let resolved = self
      .bindings
      .lock()
      .get(&type_id)
      .copied()
      .unwrap_or(self.fallback);
    self.cache.lock().insert(type_id, resolved);
    self.serializer_by_id(resolved)
  }

  /// Returns the serializer identified by id.
  #[must_use]
  pub fn serializer_by_id(&self, id: SerializerId) -> Option<ArcShared<dyn Serializer>> {
    self.serializers.lock().get(&id).cloned()
  }
}

/// Type alias for the no_std default registry.
pub type SerializationRegistry = SerializationRegistryGeneric<NoStdToolbox>;
