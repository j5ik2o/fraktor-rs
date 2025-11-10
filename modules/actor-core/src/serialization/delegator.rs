//! Delegator helper that allows serializers to perform nested serialization.

#[cfg(test)]
mod tests;

use alloc::string::ToString;
use core::any::Any;

use crate::RuntimeToolbox;

use super::{
  call_scope::SerializationCallScope,
  error::SerializationError,
  serialized_message::SerializedMessage,
  serialization_registry::SerializationRegistryGeneric,
  transport_information::TransportInformation,
};

/// Helper that routes nested serialization requests through the registry.
pub struct SerializationDelegator<'a, TB: RuntimeToolbox> {
  registry:       &'a SerializationRegistryGeneric<TB>,
  scope:          SerializationCallScope,
  transport_hint: Option<TransportInformation>,
}

impl<'a, TB: RuntimeToolbox> SerializationDelegator<'a, TB> {
  /// Creates a new delegator bound to the provided registry.
  #[must_use]
  pub const fn new(registry: &'a SerializationRegistryGeneric<TB>) -> Self {
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
  pub fn serialize(
    &self,
    value: &(dyn Any + Send + Sync),
    type_name: &str,
  ) -> Result<SerializedMessage, SerializationError> {
    let serializer = self.registry.serializer_for_type(value.type_id(), type_name, self.transport_hint.clone())?;
    let bytes = serializer.to_binary(value)?;
    let manifest = if serializer.include_manifest() { Some(type_name.to_string()) } else { None };
    Ok(SerializedMessage::new(serializer.identifier(), manifest, bytes))
  }

  /// Returns the currently configured scope.
  #[must_use]
  pub const fn scope(&self) -> SerializationCallScope {
    self.scope
  }
}
