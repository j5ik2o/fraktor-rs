//! Bridge between cluster message metadata and actor-core serialization.

#[cfg(test)]
#[path = "actor_serialization_bridge_test.rs"]
mod tests;

use alloc::boxed::Box;
use core::any::{Any, TypeId};

use fraktor_actor_core_kernel_rs::serialization::{SerializationCallScope, SerializationError, SerializationExtension};

use super::{ClusterMessagePayloadKind, ClusterSerializedMessage};

/// Connects cluster payload kind metadata to actor-core serialization.
pub trait ActorSerializationBridge {
  /// Serializes a typed cluster payload through actor-core serialization.
  ///
  /// # Errors
  ///
  /// Returns the actor-core [`SerializationError`] from the underlying serialization extension.
  fn serialize_cluster_message(
    &self,
    kind: ClusterMessagePayloadKind,
    scope: SerializationCallScope,
    message: &(dyn Any + Send + Sync),
  ) -> Result<ClusterSerializedMessage, SerializationError>;

  /// Deserializes a cluster serialized message through actor-core serialization.
  ///
  /// # Errors
  ///
  /// Returns the actor-core [`SerializationError`] from the underlying serialization extension.
  fn deserialize_cluster_message(
    &self,
    message: &ClusterSerializedMessage,
    type_hint: Option<TypeId>,
  ) -> Result<Box<dyn Any + Send + Sync>, SerializationError>;
}

impl ActorSerializationBridge for SerializationExtension {
  fn serialize_cluster_message(
    &self,
    kind: ClusterMessagePayloadKind,
    scope: SerializationCallScope,
    message: &(dyn Any + Send + Sync),
  ) -> Result<ClusterSerializedMessage, SerializationError> {
    let serialized_message = self.serialize(message, scope)?;
    Ok(ClusterSerializedMessage::new(kind, serialized_message))
  }

  fn deserialize_cluster_message(
    &self,
    message: &ClusterSerializedMessage,
    type_hint: Option<TypeId>,
  ) -> Result<Box<dyn Any + Send + Sync>, SerializationError> {
    self.deserialize(message.serialized_message(), type_hint)
  }
}
