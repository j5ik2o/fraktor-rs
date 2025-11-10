//! Builder DSL for serialization setup.

#[cfg(test)]
mod tests;

use alloc::{collections::BTreeMap, string::String, vec::Vec};
use core::any::{type_name, TypeId};

use cellactor_utils_core_rs::sync::ArcShared;
use hashbrown::HashMap;

use super::{
  builder_error::SerializationBuilderError, call_scope::SerializationCallScope,
  config_adapter::SerializationConfigAdapter, serialization_setup::SerializationSetup, serializer::Serializer,
  serializer_id::SerializerId,
};

/// Fluent builder to assemble serialization components prior to runtime initialization.
pub struct SerializationSetupBuilder {
  serializers_by_id: HashMap<SerializerId, ArcShared<dyn Serializer>>,
  serializer_ids:    HashMap<String, SerializerId>,
  bindings:          HashMap<TypeId, SerializerId>,
  binding_names:     HashMap<TypeId, String>,
  manifest_strings:  HashMap<TypeId, String>,
  routes:            HashMap<String, BTreeMap<u8, SerializerId>>,
  fallback:          Option<SerializerId>,
  scopes:            Vec<SerializationCallScope>,
  adapter_metadata:  Vec<String>,
}

impl SerializationSetupBuilder {
  /// Creates an empty builder instance.
  #[must_use]
  pub fn new() -> Self {
    Self {
      serializers_by_id: HashMap::new(),
      serializer_ids: HashMap::new(),
      bindings: HashMap::new(),
      binding_names: HashMap::new(),
      manifest_strings: HashMap::new(),
      routes: HashMap::new(),
      fallback: None,
      scopes: Vec::new(),
      adapter_metadata: Vec::new(),
    }
  }

  /// Registers a serializer implementation with the specified identifier.
  ///
  /// # Errors
  ///
  /// Returns [`SerializationBuilderError::ReservedIdentifier`] if the id collides with the runtime range.
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
  pub fn set_fallback(mut self, name: &str) -> Result<Self, SerializationBuilderError> {
    let Some(id) = self.serializer_ids.get(name).copied() else {
      return Err(SerializationBuilderError::UnknownSerializer(name.into()));
    };
    self.fallback = Some(id);
    Ok(self)
  }

  /// Binds a marker type to the named serializer.
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
  pub fn bind_remote_manifest<T: 'static>(mut self, manifest: impl Into<String>) -> Result<Self, SerializationBuilderError> {
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
  pub fn require_manifest_for_scope(mut self, scope: SerializationCallScope) -> Self {
    if !self.scopes.contains(&scope) {
      self.scopes.push(scope);
    }
    self
  }

  /// Registers a manifest evolution route for deserialization.
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
    let entry = self.routes.entry(manifest_str.clone()).or_insert_with(BTreeMap::new);
    if entry.contains_key(&priority) {
      return Err(SerializationBuilderError::ManifestRouteDuplicate { manifest: manifest_str, priority });
    }
    entry.insert(priority, serializer_id);
    Ok(self)
  }

  /// Applies an external configuration adapter to the builder.
  pub fn apply_adapter(self, adapter: &impl SerializationConfigAdapter) -> Result<Self, SerializationBuilderError> {
    let metadata = adapter.metadata();
    let mut builder = adapter.apply(self)?;
    builder.adapter_metadata.push(metadata.into());
    Ok(builder)
  }

  /// Finalizes the setup.
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
    let manifest_required = scopes.iter().any(|scope| matches!(scope, SerializationCallScope::Remote | SerializationCallScope::Persistence));
    if manifest_required {
      if let Some((_type_id, _name)) = binding_names.iter().find(|(type_id, _)| {
        let requested = **type_id;
        !manifest_strings.iter().any(|(registered, _)| *registered == requested)
      }) {
        let scope = *scopes
          .iter()
          .find(|scope| matches!(scope, SerializationCallScope::Remote | SerializationCallScope::Persistence))
          .unwrap_or(&SerializationCallScope::Remote);
        return Err(SerializationBuilderError::ManifestRequired(scope));
      }
    }
    let manifest_routes = routes
      .into_iter()
      .map(|(manifest, ordered)| {
        let entries = ordered.into_iter().collect::<Vec<_>>();
        (manifest, entries)
      })
      .collect();
    Ok(SerializationSetup::from_parts(
      serializers_by_id,
      bindings,
      manifest_strings,
      manifest_routes,
      scopes,
      fallback,
      adapter_metadata,
    ))
  }
}
