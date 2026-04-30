//! Delegator helper that allows serializers to perform nested serialization.

#[cfg(test)]
mod tests;

use alloc::{boxed::Box, string::String};
use core::any::{Any, TypeId};

use super::{
  call_scope::SerializationCallScope, error::SerializationError, serialization_registry::SerializationRegistry,
  serialized_message::SerializedMessage, transport_information::TransportInformation,
};

/// Helper that routes nested serialization requests through the registry.
pub struct SerializationDelegator<'a> {
  registry:       &'a SerializationRegistry,
  scope:          SerializationCallScope,
  transport_hint: Option<TransportInformation>,
}

impl<'a> SerializationDelegator<'a> {
  /// Creates a new delegator bound to the provided registry.
  #[must_use]
  pub const fn new(registry: &'a SerializationRegistry) -> Self {
    Self { registry, scope: SerializationCallScope::Local, transport_hint: None }
  }

  /// Updates the call scope used when resolving serializer requirements.
  #[must_use]
  pub const fn with_scope(mut self, scope: SerializationCallScope) -> Self {
    self.scope = scope;
    self
  }

  /// Attaches a transport hint that will be propagated to registry errors.
  #[must_use]
  pub fn with_transport_information(mut self, info: TransportInformation) -> Self {
    self.transport_hint = Some(info);
    self
  }

  /// Serializes the nested payload using the registry configuration.
  ///
  /// # Errors
  ///
  /// Returns an error if:
  /// - No serializer is found for the given type
  /// - The serialization process fails
  pub fn serialize(
    &self,
    value: &(dyn Any + Send + Sync),
    type_name: &str,
  ) -> Result<SerializedMessage, SerializationError> {
    let (serializer, _) = self.registry.serializer_for_type(value.type_id(), type_name, self.transport_hint.clone())?;
    let bytes = serializer.to_binary(value)?;
    let manifest = self
      .registry
      .manifest_for(value.type_id())
      .map(String::from)
      .or_else(|| serializer.as_string_manifest().map(|provider| provider.manifest(value).into_owned()));
    Ok(SerializedMessage::new(serializer.identifier(), manifest, bytes))
  }

  /// Deserializes a nested payload using the registry configuration.
  ///
  /// # Errors
  ///
  /// Returns an error if:
  /// - The serializer ID is not registered
  /// - The payload format is invalid for the resolved serializer
  /// - Manifest routing cannot resolve the payload
  pub fn deserialize(
    &self,
    message: &SerializedMessage,
    type_hint: Option<TypeId>,
  ) -> Result<Box<dyn Any + Send + Sync>, SerializationError> {
    let serializer = self.registry.serializer_by_id(message.serializer_id())?;
    let result = if let Some(manifest) = message.manifest()
      && let Some(provider) = serializer.as_string_manifest()
    {
      provider.from_binary_with_manifest(message.bytes(), manifest)
    } else {
      serializer.from_binary(message.bytes(), type_hint)
    };
    match result {
      | Ok(value) => Ok(value),
      | Err(SerializationError::UnknownManifest(manifest)) => {
        self.deserialize_with_manifest_routes(message, manifest, type_hint)
      },
      | Err(error) => Err(error),
    }
  }

  /// Returns the currently configured scope.
  #[must_use]
  pub const fn scope(&self) -> SerializationCallScope {
    self.scope
  }

  /// Iterates serializers that have explicitly opted into the given manifest via
  /// [`SerializationRegistry::serializers_for_manifest`] and returns the first successful decode.
  ///
  /// This is the manifest-aliasing fallback: when the originally-targeted serializer
  /// returns [`SerializationError::UnknownManifest`], any serializer registered against the
  /// same manifest string (e.g. via [`builder::register_manifest_route`]) is re-tried.
  ///
  /// **Invariant:** every serializer reachable here is expected to share a compatible wire
  /// format for `manifest`; the registry honours this by gating registration through
  /// `register_manifest_route`. Routes registered against incompatible types will surface
  /// as `Err(SerializationError::UnknownManifest)` per call and are skipped via `continue`.
  /// If no route succeeds, the original `UnknownManifest(manifest)` error is propagated.
  fn deserialize_with_manifest_routes(
    &self,
    message: &SerializedMessage,
    manifest: String,
    type_hint: Option<TypeId>,
  ) -> Result<Box<dyn Any + Send + Sync>, SerializationError> {
    for serializer in self.registry.serializers_for_manifest(&manifest) {
      let outcome = if let Some(provider) = serializer.as_string_manifest() {
        provider.from_binary_with_manifest(message.bytes(), &manifest)
      } else {
        serializer.from_binary(message.bytes(), type_hint)
      };
      match outcome {
        | Ok(value) => return Ok(value),
        | Err(SerializationError::UnknownManifest(_)) => continue,
        | Err(error) => return Err(error),
      }
    }
    Err(SerializationError::UnknownManifest(manifest))
  }
}
