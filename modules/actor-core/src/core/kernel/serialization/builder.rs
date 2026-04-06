//! Builder DSL for serialization setup.

#[cfg(test)]
mod tests;

use alloc::{collections::BTreeMap, string::String, vec::Vec};
use core::any::{TypeId, type_name};

use ahash::RandomState;
use fraktor_utils_rs::core::sync::ArcShared;
use hashbrown::HashMap;

use super::{
  builder_error::SerializationBuilderError, call_scope::SerializationCallScope,
  config_adapter::SerializationConfigAdapter, serialization_setup::SerializationSetup, serializer::Serializer,
  serializer_id::SerializerId,
};

/// Fluent builder to assemble serialization components prior to runtime initialization.
pub struct SerializationSetupBuilder {
  serializers_by_id: HashMap<SerializerId, ArcShared<dyn Serializer>, RandomState>,
  serializer_ids:    HashMap<String, SerializerId, RandomState>,
  bindings:          HashMap<TypeId, SerializerId, RandomState>,
  binding_names:     HashMap<TypeId, String, RandomState>,
  manifest_strings:  HashMap<TypeId, String, RandomState>,
  routes:            HashMap<String, BTreeMap<u8, SerializerId>, RandomState>,
  fallback:          Option<SerializerId>,
  scopes:            Vec<SerializationCallScope>,
  adapter_metadata:  Vec<String>,
}

impl Default for SerializationSetupBuilder {
  fn default() -> Self {
    Self::new()
  }
}

impl SerializationSetupBuilder {
  /// Creates an empty builder instance.
  #[must_use]
  pub fn new() -> Self {
    Self {
      serializers_by_id: HashMap::with_hasher(RandomState::new()),
      serializer_ids:    HashMap::with_hasher(RandomState::new()),
      bindings:          HashMap::with_hasher(RandomState::new()),
      binding_names:     HashMap::with_hasher(RandomState::new()),
      manifest_strings:  HashMap::with_hasher(RandomState::new()),
      routes:            HashMap::with_hasher(RandomState::new()),
      fallback:          None,
      scopes:            Vec::new(),
      adapter_metadata:  Vec::new(),
    }
  }

  /// Registers a serializer implementation with the specified identifier.
  ///
  /// # Errors
  ///
  /// Returns [`SerializationBuilderError::ReservedIdentifier`] if the id collides with the runtime
  /// range.
  pub fn register_serializer(
    mut self,
    name: impl Into<String>,
    id: SerializerId,
    serializer: ArcShared<dyn Serializer>,
  ) -> Result<Self, SerializationBuilderError> {
    let name = name.into();
    if id.value() <= 40 {
      return Err(SerializationBuilderError::ReservedIdentifier(id.value()));
    }
    if self.serializer_ids.contains_key(&name) {
      return Err(SerializationBuilderError::DuplicateName(name));
    }
    if self.serializers_by_id.contains_key(&id) {
      return Err(SerializationBuilderError::DuplicateIdentifier(id));
    }
    self.serializer_ids.insert(name, id);
    self.serializers_by_id.insert(id, serializer);
    Ok(self)
  }

  /// Marks the named serializer as the fallback implementation.
  ///
  /// # Errors
  ///
  /// Returns [`SerializationBuilderError::UnknownSerializer`] if the specified serializer name is
  /// not registered.
  pub fn set_fallback(mut self, name: &str) -> Result<Self, SerializationBuilderError> {
    let Some(id) = self.serializer_ids.get(name).copied() else {
      return Err(SerializationBuilderError::UnknownSerializer(name.into()));
    };
    self.fallback = Some(id);
    Ok(self)
  }

  /// Binds a marker type to the named serializer.
  ///
  /// # Errors
  ///
  /// Returns [`SerializationBuilderError::DuplicateMarker`] if the marker type is already bound.
  /// Returns [`SerializationBuilderError::UnknownSerializer`] if the specified serializer name is
  /// not registered.
  pub fn bind<T: 'static>(mut self, name: &str) -> Result<Self, SerializationBuilderError> {
    let type_id = TypeId::of::<T>();
    let type_name = type_name::<T>().into();
    if self.bindings.contains_key(&type_id) {
      return Err(SerializationBuilderError::DuplicateMarker(type_name));
    }
    let Some(serializer_id) = self.serializer_ids.get(name).copied() else {
      return Err(SerializationBuilderError::UnknownSerializer(name.into()));
    };
    self.bindings.insert(type_id, serializer_id);
    self.binding_names.insert(type_id, type_name);
    Ok(self)
  }

  /// Associates a logical manifest string with the marker type.
  ///
  /// # Errors
  ///
  /// Returns [`SerializationBuilderError::MarkerUnbound`] if the marker type is not yet bound to a
  /// serializer. Returns [`SerializationBuilderError::DuplicateManifestBinding`] if the marker
  /// type already has a manifest string.
  pub fn bind_remote_manifest<T: 'static>(
    mut self,
    manifest: impl Into<String>,
  ) -> Result<Self, SerializationBuilderError> {
    let type_id = TypeId::of::<T>();
    let type_name = String::from(type_name::<T>());
    if !self.bindings.contains_key(&type_id) {
      return Err(SerializationBuilderError::MarkerUnbound(type_name));
    }
    if self.manifest_strings.contains_key(&type_id) {
      return Err(SerializationBuilderError::DuplicateManifestBinding(type_name));
    }
    self.manifest_strings.insert(type_id, manifest.into());
    Ok(self)
  }

  /// Requires manifests for the given scope.
  #[must_use]
  pub fn require_manifest_for_scope(mut self, scope: SerializationCallScope) -> Self {
    if !self.scopes.contains(&scope) {
      self.scopes.push(scope);
    }
    self
  }

  /// Registers a manifest evolution route for deserialization.
  ///
  /// # Errors
  ///
  /// Returns [`SerializationBuilderError::UnknownSerializer`] if the specified serializer name is
  /// not registered. Returns [`SerializationBuilderError::ManifestRouteDuplicate`] if the manifest
  /// already has a route with the same priority.
  pub fn register_manifest_route(
    mut self,
    manifest: impl Into<String>,
    priority: u8,
    serializer_name: &str,
  ) -> Result<Self, SerializationBuilderError> {
    let manifest_str = manifest.into();
    let Some(serializer_id) = self.serializer_ids.get(serializer_name).copied() else {
      return Err(SerializationBuilderError::UnknownSerializer(serializer_name.into()));
    };
    let entry = self.routes.entry(manifest_str.clone()).or_default();
    if entry.contains_key(&priority) {
      return Err(SerializationBuilderError::ManifestRouteDuplicate { manifest: manifest_str, priority });
    }
    entry.insert(priority, serializer_id);
    Ok(self)
  }

  /// Applies an external configuration adapter to the builder.
  ///
  /// # Errors
  ///
  /// Returns errors propagated from the adapter's `apply` method.
  pub fn apply_adapter(self, adapter: &impl SerializationConfigAdapter) -> Result<Self, SerializationBuilderError> {
    let metadata = adapter.metadata();
    let mut builder = adapter.apply(self)?;
    builder.adapter_metadata.push(metadata.into());
    Ok(builder)
  }

  /// Finalizes the setup.
  ///
  /// # Errors
  ///
  /// Returns [`SerializationBuilderError::MissingFallback`] if no fallback serializer was set.
  /// Returns [`SerializationBuilderError::ManifestRequired`] if a scope requires manifests but one
  /// or more bound types lack manifest strings.
  pub fn build(self) -> Result<SerializationSetup, SerializationBuilderError> {
    let Self {
      serializers_by_id,
      serializer_ids: _,
      bindings,
      binding_names,
      manifest_strings,
      routes,
      fallback,
      scopes,
      adapter_metadata,
    } = self;
    let fallback = fallback.ok_or(SerializationBuilderError::MissingFallback)?;
    let manifest_required =
      scopes.iter().any(|scope| matches!(scope, SerializationCallScope::Remote | SerializationCallScope::Persistence));
    if manifest_required
      && let Some((_type_id, _name)) = binding_names.iter().find(|(type_id, _)| {
        let requested = **type_id;
        !manifest_strings.contains_key(&requested)
      })
    {
      let scope = *scopes
        .iter()
        .find(|scope| matches!(scope, SerializationCallScope::Remote | SerializationCallScope::Persistence))
        .unwrap_or(&SerializationCallScope::Remote);
      return Err(SerializationBuilderError::ManifestRequired(scope));
    }
    let mut manifest_routes = HashMap::with_hasher(RandomState::new());
    manifest_routes.extend(routes.into_iter().map(|(manifest, ordered)| {
      let entries = ordered.into_iter().collect::<Vec<_>>();
      (manifest, entries)
    }));
    Ok(SerializationSetup::from_parts(
      serializers_by_id,
      bindings,
      binding_names,
      manifest_strings,
      manifest_routes,
      scopes,
      fallback,
      adapter_metadata,
    ))
  }
}
