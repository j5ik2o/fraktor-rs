//! Immutable serialization setup produced by the builder.

#[cfg(test)]
mod tests;

use alloc::{string::String, vec::Vec};
use core::any::TypeId;

use ahash::RandomState;
use fraktor_utils_rs::core::sync::ArcShared;
use hashbrown::HashMap;

use super::{call_scope::SerializationCallScope, serializer::Serializer, serializer_id::SerializerId};

/// Snapshot of serialization configuration applied to the actor system.
#[derive(Clone)]
pub struct SerializationSetup {
  serializers:      HashMap<SerializerId, ArcShared<dyn Serializer>, RandomState>,
  bindings:         HashMap<TypeId, SerializerId, RandomState>,
  binding_names:    HashMap<TypeId, String, RandomState>,
  remote_manifests: HashMap<TypeId, String, RandomState>,
  manifest_routes:  HashMap<String, Vec<(u8, SerializerId)>, RandomState>,
  scopes:           Vec<SerializationCallScope>,
  fallback:         SerializerId,
  adapter_metadata: Vec<String>,
}

impl SerializationSetup {
  /// Creates a setup instance from builder-owned data.
  #[must_use]
  #[allow(clippy::too_many_arguments)]
  pub(crate) fn from_parts(
    serializers: HashMap<SerializerId, ArcShared<dyn Serializer>, RandomState>,
    bindings: HashMap<TypeId, SerializerId, RandomState>,
    binding_names: HashMap<TypeId, String, RandomState>,
    remote_manifests: HashMap<TypeId, String, RandomState>,
    manifest_routes: HashMap<String, Vec<(u8, SerializerId)>, RandomState>,
    scopes: Vec<SerializationCallScope>,
    fallback: SerializerId,
    adapter_metadata: Vec<String>,
  ) -> Self {
    Self { serializers, bindings, binding_names, remote_manifests, manifest_routes, scopes, fallback, adapter_metadata }
  }

  /// Returns the serializer bound to the provided type identifier.
  #[must_use]
  pub fn binding_for(&self, type_id: TypeId) -> Option<SerializerId> {
    self.bindings.get(&type_id).copied()
  }

  /// Returns the manifest associated with the type if one was registered.
  #[must_use]
  pub fn manifest_for(&self, type_id: TypeId) -> Option<&str> {
    self.remote_manifests.get(&type_id).map(String::as_str)
  }

  /// Returns the scopes that require manifests.
  #[must_use]
  #[allow(clippy::missing_const_for_fn)] // Vec の Deref が const でないため const fn にできない
  pub fn manifest_required_scopes(&self) -> &[SerializationCallScope] {
    &self.scopes
  }

  /// Returns the recorded type name for the binding (if provided).
  #[must_use]
  pub fn binding_name(&self, type_id: TypeId) -> Option<&str> {
    self.binding_names.get(&type_id).map(String::as_str)
  }

  /// Returns the fallback serializer identifier.
  #[must_use]
  pub const fn fallback_serializer(&self) -> SerializerId {
    self.fallback
  }

  /// Returns serialized manifest routes.
  #[must_use]
  pub const fn manifest_routes(&self) -> &HashMap<String, Vec<(u8, SerializerId)>, RandomState> {
    &self.manifest_routes
  }

  /// Returns metadata recorded while applying adapters.
  #[must_use]
  #[allow(clippy::missing_const_for_fn)] // Vec の Deref が const でないため const fn にできない
  pub fn adapter_metadata(&self) -> &[String] {
    &self.adapter_metadata
  }

  /// Returns the serializer associated with the identifier.
  #[must_use]
  pub fn serializer(&self, id: &SerializerId) -> Option<&ArcShared<dyn Serializer>> {
    self.serializers.get(id)
  }

  /// Returns the internal serializer mapping (crate visibility for registry construction).
  pub(crate) fn serializers_ref(&self) -> &HashMap<SerializerId, ArcShared<dyn Serializer>, RandomState> {
    &self.serializers
  }

  /// Returns the binding map (crate visibility).
  pub(crate) const fn bindings_ref(&self) -> &HashMap<TypeId, SerializerId, RandomState> {
    &self.bindings
  }

  /// Returns the binding names map (crate visibility).
  pub(crate) const fn binding_names_ref(&self) -> &HashMap<TypeId, String, RandomState> {
    &self.binding_names
  }

  /// Returns manifest routes (crate visibility).
  pub(crate) const fn manifest_routes_ref(&self) -> &HashMap<String, Vec<(u8, SerializerId)>, RandomState> {
    &self.manifest_routes
  }
}
