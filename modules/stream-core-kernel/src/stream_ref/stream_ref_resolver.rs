#[cfg(test)]
#[path = "stream_ref_resolver_test.rs"]
mod tests;

use alloc::{format, string::String};
use core::any::{Any, type_name};

use fraktor_actor_core_kernel_rs::{
  actor::{actor_path::ActorPathParser, actor_ref::ActorRef},
  serialization::{
    NotSerializableError, SerializationError, SerializedMessage, Serializer, SerializerWithStringManifest,
  },
  system::ActorSystem,
};

use super::{
  SINK_REF_MANIFEST, SOURCE_REF_MANIFEST, STREAM_REF_PROTOCOL_SERIALIZER_ID, SinkRef, SourceRef,
  StreamRefProtocolSerializer, StreamRefSinkRefPayload, StreamRefSourceRefPayload,
};
use crate::StreamError;

/// Low-level StreamRef resolver support for serializer implementations.
///
/// This type intentionally exposes canonical actor path strings only at the
/// serializer/resolver boundary. Application-level handoff should keep passing
/// typed [`SourceRef`] and [`SinkRef`] values.
pub struct StreamRefResolver {
  system: ActorSystem,
}

impl StreamRefResolver {
  /// Creates a resolver bound to an actor system.
  #[must_use]
  pub const fn new(system: ActorSystem) -> Self {
    Self { system }
  }

  /// Converts an actor-backed [`SourceRef`] to its canonical serialization format.
  ///
  /// # Errors
  ///
  /// Returns [`StreamError`] when the ref has no materialized endpoint actor or
  /// the endpoint actor has no canonical path.
  pub fn source_ref_to_format<T>(&self, source_ref: &SourceRef<T>) -> Result<String, StreamError> {
    let actor_ref = source_ref.endpoint_actor_ref()?;
    let canonical = source_ref.canonical_actor_path()?;
    debug_assert_eq!(Self::actor_ref_to_format(&actor_ref)?, canonical);
    Ok(canonical)
  }

  /// Converts an actor-backed [`SinkRef`] to its canonical serialization format.
  ///
  /// # Errors
  ///
  /// Returns [`StreamError`] when the ref has no materialized endpoint actor or
  /// the endpoint actor has no canonical path.
  pub fn sink_ref_to_format<T>(&self, sink_ref: &SinkRef<T>) -> Result<String, StreamError> {
    let actor_ref = sink_ref.endpoint_actor_ref()?;
    let canonical = sink_ref.canonical_actor_path()?;
    debug_assert_eq!(Self::actor_ref_to_format(&actor_ref)?, canonical);
    Ok(canonical)
  }

  /// Resolves a serialized [`SourceRef`] through the actor-ref provider surface.
  ///
  /// # Errors
  ///
  /// Returns [`StreamError`] when the format is not an actor path or provider
  /// dispatch cannot resolve the endpoint actor reference.
  pub fn resolve_source_ref<T>(&self, serialized: &str) -> Result<SourceRef<T>, StreamError> {
    let actor_ref = self.resolve_actor_ref(serialized)?;
    Ok(SourceRef::from_endpoint_actor(actor_ref))
  }

  /// Resolves a serialized [`SinkRef`] through the actor-ref provider surface.
  ///
  /// # Errors
  ///
  /// Returns [`StreamError`] when the format is not an actor path or provider
  /// dispatch cannot resolve the endpoint actor reference.
  pub fn resolve_sink_ref<T>(&self, serialized: &str) -> Result<SinkRef<T>, StreamError> {
    let actor_ref = self.resolve_actor_ref(serialized)?;
    Ok(SinkRef::from_endpoint_actor(actor_ref))
  }

  /// Converts an actor-backed [`SourceRef`] to a nested serialized payload for domain serializers.
  ///
  /// # Errors
  ///
  /// Returns [`SerializationError`] when the ref has no materialized endpoint actor or the
  /// serialized payload cannot be encoded.
  pub fn source_ref_to_serialized_message<T>(
    &self,
    source_ref: &SourceRef<T>,
  ) -> Result<SerializedMessage, SerializationError> {
    let actor_path = self
      .source_ref_to_format(source_ref)
      .map_err(|error| Self::ref_not_serializable(type_name::<SourceRef<T>>(), SOURCE_REF_MANIFEST, error))?;
    let payload = StreamRefSourceRefPayload::new(actor_path);
    Self::payload_to_serialized_message(&payload, SOURCE_REF_MANIFEST)
  }

  /// Converts an actor-backed [`SinkRef`] to a nested serialized payload for domain serializers.
  ///
  /// # Errors
  ///
  /// Returns [`SerializationError`] when the ref has no materialized endpoint actor or the
  /// serialized payload cannot be encoded.
  pub fn sink_ref_to_serialized_message<T>(
    &self,
    sink_ref: &SinkRef<T>,
  ) -> Result<SerializedMessage, SerializationError> {
    let actor_path = self
      .sink_ref_to_format(sink_ref)
      .map_err(|error| Self::ref_not_serializable(type_name::<SinkRef<T>>(), SINK_REF_MANIFEST, error))?;
    let payload = StreamRefSinkRefPayload::new(actor_path);
    Self::payload_to_serialized_message(&payload, SINK_REF_MANIFEST)
  }

  /// Resolves a nested SourceRef payload through provider dispatch.
  ///
  /// # Errors
  ///
  /// Returns [`SerializationError`] when the serialized payload is not a SourceRef payload or the
  /// endpoint actor cannot be resolved.
  pub fn resolve_source_ref_message<T>(&self, message: &SerializedMessage) -> Result<SourceRef<T>, SerializationError> {
    let payload = Self::serialized_message_to_source_ref_payload(message)?;
    self
      .resolve_source_ref(payload.actor_path())
      .map_err(|error| Self::ref_not_serializable(type_name::<SourceRef<T>>(), SOURCE_REF_MANIFEST, error))
  }

  /// Resolves a nested SinkRef payload through provider dispatch.
  ///
  /// # Errors
  ///
  /// Returns [`SerializationError`] when the serialized payload is not a SinkRef payload or the
  /// endpoint actor cannot be resolved.
  pub fn resolve_sink_ref_message<T>(&self, message: &SerializedMessage) -> Result<SinkRef<T>, SerializationError> {
    let payload = Self::serialized_message_to_sink_ref_payload(message)?;
    self
      .resolve_sink_ref(payload.actor_path())
      .map_err(|error| Self::ref_not_serializable(type_name::<SinkRef<T>>(), SINK_REF_MANIFEST, error))
  }

  fn actor_ref_to_format(actor_ref: &ActorRef) -> Result<String, StreamError> {
    actor_ref.canonical_path().map(|path| path.to_canonical_uri()).ok_or(StreamError::StreamRefTargetNotInitialized)
  }

  fn resolve_actor_ref(&self, serialized: &str) -> Result<ActorRef, StreamError> {
    let actor_path = ActorPathParser::parse(serialized)
      .map_err(|error| StreamError::failed_with_context(format!("invalid StreamRef actor path: {error}")))?;
    self
      .system
      .resolve_actor_ref(actor_path)
      .map_err(|error| StreamError::failed_with_context(format!("StreamRef provider dispatch failed: {error}")))
  }

  fn payload_to_serialized_message(
    payload: &(dyn Any + Send + Sync),
    expected_manifest: &'static str,
  ) -> Result<SerializedMessage, SerializationError> {
    let serializer = StreamRefProtocolSerializer::new(STREAM_REF_PROTOCOL_SERIALIZER_ID);
    let bytes = serializer.to_binary(payload)?;
    let manifest = serializer.as_string_manifest().map(|provider| provider.manifest(payload).into_owned());
    match manifest {
      | Some(manifest) if manifest == expected_manifest => {
        Ok(SerializedMessage::new(STREAM_REF_PROTOCOL_SERIALIZER_ID, Some(manifest), bytes))
      },
      | _ => Err(SerializationError::InvalidFormat),
    }
  }

  fn serialized_message_to_source_ref_payload(
    message: &SerializedMessage,
  ) -> Result<StreamRefSourceRefPayload, SerializationError> {
    Self::ensure_ref_payload_message(message, SOURCE_REF_MANIFEST)?;
    let serializer = StreamRefProtocolSerializer::new(STREAM_REF_PROTOCOL_SERIALIZER_ID);
    let payload = serializer.from_binary_with_manifest(message.bytes(), SOURCE_REF_MANIFEST)?;
    payload
      .downcast::<StreamRefSourceRefPayload>()
      .map(|payload| *payload)
      .map_err(|_| SerializationError::InvalidFormat)
  }

  fn serialized_message_to_sink_ref_payload(
    message: &SerializedMessage,
  ) -> Result<StreamRefSinkRefPayload, SerializationError> {
    Self::ensure_ref_payload_message(message, SINK_REF_MANIFEST)?;
    let serializer = StreamRefProtocolSerializer::new(STREAM_REF_PROTOCOL_SERIALIZER_ID);
    let payload = serializer.from_binary_with_manifest(message.bytes(), SINK_REF_MANIFEST)?;
    payload.downcast::<StreamRefSinkRefPayload>().map(|payload| *payload).map_err(|_| SerializationError::InvalidFormat)
  }

  fn ensure_ref_payload_message(
    message: &SerializedMessage,
    expected_manifest: &'static str,
  ) -> Result<(), SerializationError> {
    if message.serializer_id() != STREAM_REF_PROTOCOL_SERIALIZER_ID {
      return Err(SerializationError::UnknownSerializer(message.serializer_id()));
    }
    let Some(manifest) = message.manifest() else {
      return Err(SerializationError::InvalidFormat);
    };
    if manifest == expected_manifest {
      return Ok(());
    }
    Err(SerializationError::UnknownManifest(String::from(manifest)))
  }

  fn ref_not_serializable(type_name: &'static str, manifest: &'static str, _error: StreamError) -> SerializationError {
    let payload = NotSerializableError::new(
      type_name,
      Some(STREAM_REF_PROTOCOL_SERIALIZER_ID),
      Some(String::from(manifest)),
      None,
      None,
    );
    SerializationError::NotSerializable(payload)
  }
}
