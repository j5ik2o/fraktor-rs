//! Registry for resolving event adapters by event type.

#[cfg(test)]
mod tests;

use core::{
  any::{Any, TypeId},
  fmt::{Debug, Formatter},
};

use ahash::RandomState;
use fraktor_utils_rs::core::sync::ArcShared;
use hashbrown::HashMap;

use crate::core::{
  event_seq::EventSeq, identity_event_adapter::IdentityEventAdapter, read_event_adapter::ReadEventAdapter,
  write_event_adapter::WriteEventAdapter,
};

/// Registry that resolves write/read adapters by event type.
#[derive(Clone)]
pub struct EventAdapters {
  write_adapters: HashMap<TypeId, ArcShared<dyn WriteEventAdapter>, RandomState>,
  read_adapters:  HashMap<TypeId, ArcShared<dyn ReadEventAdapter>, RandomState>,
  identity:       ArcShared<IdentityEventAdapter>,
}

impl EventAdapters {
  /// Creates an empty adapter registry with identity fallback.
  #[must_use]
  pub fn new() -> Self {
    Self {
      write_adapters: HashMap::with_hasher(RandomState::new()),
      read_adapters:  HashMap::with_hasher(RandomState::new()),
      identity:       ArcShared::new(IdentityEventAdapter::new()),
    }
  }

  /// Returns the number of explicit adapter bindings.
  #[must_use]
  pub fn len(&self) -> usize {
    self.write_adapters.len()
  }

  /// Returns whether the registry has no explicit bindings.
  #[must_use]
  pub fn is_empty(&self) -> bool {
    self.write_adapters.is_empty()
  }

  /// Registers adapters for the specified event type.
  pub fn register<E>(
    &mut self,
    write_adapter: ArcShared<dyn WriteEventAdapter>,
    read_adapter: ArcShared<dyn ReadEventAdapter>,
  ) where
    E: Send + Sync + 'static, {
    self.register_by_type_id(TypeId::of::<E>(), write_adapter, read_adapter);
  }

  /// Registers adapters for the specified type identifier.
  pub fn register_by_type_id(
    &mut self,
    type_id: TypeId,
    write_adapter: ArcShared<dyn WriteEventAdapter>,
    read_adapter: ArcShared<dyn ReadEventAdapter>,
  ) {
    self.write_adapters.insert(type_id, write_adapter);
    self.read_adapters.insert(type_id, read_adapter);
  }

  /// Removes adapters associated with the specified event type.
  pub fn unregister<E>(&mut self)
  where
    E: Send + Sync + 'static, {
    self.unregister_by_type_id(TypeId::of::<E>());
  }

  /// Removes adapters associated with the specified type identifier.
  pub fn unregister_by_type_id(&mut self, type_id: TypeId) {
    self.write_adapters.remove(&type_id);
    self.read_adapters.remove(&type_id);
  }

  /// Resolves a write adapter for the specified event type.
  #[must_use]
  pub fn write_adapter_for<E>(&self) -> ArcShared<dyn WriteEventAdapter>
  where
    E: Send + Sync + 'static, {
    self.write_adapter_for_type_id(TypeId::of::<E>())
  }

  /// Resolves a read adapter for the specified event type.
  #[must_use]
  pub fn read_adapter_for<E>(&self) -> ArcShared<dyn ReadEventAdapter>
  where
    E: Send + Sync + 'static, {
    self.read_adapter_for_type_id(TypeId::of::<E>())
  }

  /// Resolves a write adapter for the specified type identifier.
  #[must_use]
  pub fn write_adapter_for_type_id(&self, type_id: TypeId) -> ArcShared<dyn WriteEventAdapter> {
    self.write_adapters.get(&type_id).cloned().unwrap_or_else(|| self.identity.clone())
  }

  /// Resolves a read adapter for the specified type identifier.
  #[must_use]
  pub fn read_adapter_for_type_id(&self, type_id: TypeId) -> ArcShared<dyn ReadEventAdapter> {
    self.read_adapters.get(&type_id).cloned().unwrap_or_else(|| self.identity.clone())
  }

  /// Converts an event to journal representation using the registered write adapter.
  #[must_use]
  pub fn to_journal<E>(&self, event: ArcShared<dyn Any + Send + Sync>) -> ArcShared<dyn Any + Send + Sync>
  where
    E: Send + Sync + 'static, {
    self.write_adapter_for::<E>().to_journal(event)
  }

  /// Converts a journal payload back to domain events using the registered read adapter.
  #[must_use]
  pub fn adapt_from_journal<E>(&self, event: ArcShared<dyn Any + Send + Sync>, manifest: &str) -> EventSeq
  where
    E: Send + Sync + 'static, {
    self.read_adapter_for::<E>().adapt_from_journal(event, manifest)
  }
}

impl Default for EventAdapters {
  fn default() -> Self {
    Self::new()
  }
}

impl Debug for EventAdapters {
  fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
    f.debug_struct("EventAdapters")
      .field("write_adapter_count", &self.write_adapters.len())
      .field("read_adapter_count", &self.read_adapters.len())
      .finish()
  }
}
