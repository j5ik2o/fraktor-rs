#[cfg(test)]
#[path = "stream_ref_protocol_serialization_setup_test.rs"]
mod tests;

use fraktor_actor_core_kernel_rs::serialization::{
  SerializationBuilderError, SerializationConfigAdapter, SerializationSetupBuilder, Serializer,
};
use fraktor_utils_core_rs::sync::ArcShared;

use super::{
  ACK_MANIFEST, CUMULATIVE_DEMAND_MANIFEST, ON_SUBSCRIBE_HANDSHAKE_MANIFEST, REMOTE_STREAM_COMPLETED_MANIFEST,
  REMOTE_STREAM_FAILURE_MANIFEST, SEQUENCED_ON_NEXT_MANIFEST, SINK_REF_MANIFEST, SOURCE_REF_MANIFEST,
  STREAM_REF_PROTOCOL_SERIALIZER_ID, STREAM_REF_PROTOCOL_SERIALIZER_NAME, StreamRefAck, StreamRefCumulativeDemand,
  StreamRefOnSubscribeHandshake, StreamRefProtocolSerializer, StreamRefRemoteStreamCompleted,
  StreamRefRemoteStreamFailure, StreamRefSequencedOnNext, StreamRefSinkRefPayload, StreamRefSourceRefPayload,
};

/// Serialization setup contribution for StreamRef protocol payloads.
#[derive(Debug, Clone, Copy, Default)]
pub struct StreamRefProtocolSerializationSetup;

impl StreamRefProtocolSerializationSetup {
  /// Creates a StreamRef protocol serialization setup contribution.
  #[must_use]
  pub const fn new() -> Self {
    Self
  }

  fn bind_protocol_types(
    builder: SerializationSetupBuilder,
  ) -> Result<SerializationSetupBuilder, SerializationBuilderError> {
    let builder = builder
      .bind::<StreamRefSequencedOnNext>(STREAM_REF_PROTOCOL_SERIALIZER_NAME)?
      .bind_remote_manifest::<StreamRefSequencedOnNext>(SEQUENCED_ON_NEXT_MANIFEST)?;
    let builder = builder
      .bind::<StreamRefCumulativeDemand>(STREAM_REF_PROTOCOL_SERIALIZER_NAME)?
      .bind_remote_manifest::<StreamRefCumulativeDemand>(CUMULATIVE_DEMAND_MANIFEST)?;
    let builder = builder
      .bind::<StreamRefRemoteStreamFailure>(STREAM_REF_PROTOCOL_SERIALIZER_NAME)?
      .bind_remote_manifest::<StreamRefRemoteStreamFailure>(REMOTE_STREAM_FAILURE_MANIFEST)?;
    let builder = builder
      .bind::<StreamRefRemoteStreamCompleted>(STREAM_REF_PROTOCOL_SERIALIZER_NAME)?
      .bind_remote_manifest::<StreamRefRemoteStreamCompleted>(REMOTE_STREAM_COMPLETED_MANIFEST)?;
    let builder = builder
      .bind::<StreamRefOnSubscribeHandshake>(STREAM_REF_PROTOCOL_SERIALIZER_NAME)?
      .bind_remote_manifest::<StreamRefOnSubscribeHandshake>(ON_SUBSCRIBE_HANDSHAKE_MANIFEST)?;
    let builder = builder
      .bind::<StreamRefAck>(STREAM_REF_PROTOCOL_SERIALIZER_NAME)?
      .bind_remote_manifest::<StreamRefAck>(ACK_MANIFEST)?;
    let builder = builder
      .bind::<StreamRefSourceRefPayload>(STREAM_REF_PROTOCOL_SERIALIZER_NAME)?
      .bind_remote_manifest::<StreamRefSourceRefPayload>(SOURCE_REF_MANIFEST)?;
    builder
      .bind::<StreamRefSinkRefPayload>(STREAM_REF_PROTOCOL_SERIALIZER_NAME)?
      .bind_remote_manifest::<StreamRefSinkRefPayload>(SINK_REF_MANIFEST)
  }

  fn register_manifest_routes(
    builder: SerializationSetupBuilder,
  ) -> Result<SerializationSetupBuilder, SerializationBuilderError> {
    let builder =
      builder.register_manifest_route(SEQUENCED_ON_NEXT_MANIFEST, 0, STREAM_REF_PROTOCOL_SERIALIZER_NAME)?;
    let builder =
      builder.register_manifest_route(CUMULATIVE_DEMAND_MANIFEST, 0, STREAM_REF_PROTOCOL_SERIALIZER_NAME)?;
    let builder =
      builder.register_manifest_route(REMOTE_STREAM_FAILURE_MANIFEST, 0, STREAM_REF_PROTOCOL_SERIALIZER_NAME)?;
    let builder =
      builder.register_manifest_route(REMOTE_STREAM_COMPLETED_MANIFEST, 0, STREAM_REF_PROTOCOL_SERIALIZER_NAME)?;
    let builder =
      builder.register_manifest_route(ON_SUBSCRIBE_HANDSHAKE_MANIFEST, 0, STREAM_REF_PROTOCOL_SERIALIZER_NAME)?;
    let builder = builder.register_manifest_route(ACK_MANIFEST, 0, STREAM_REF_PROTOCOL_SERIALIZER_NAME)?;
    let builder = builder.register_manifest_route(SOURCE_REF_MANIFEST, 0, STREAM_REF_PROTOCOL_SERIALIZER_NAME)?;
    builder.register_manifest_route(SINK_REF_MANIFEST, 0, STREAM_REF_PROTOCOL_SERIALIZER_NAME)
  }
}

impl SerializationConfigAdapter for StreamRefProtocolSerializationSetup {
  fn apply(&self, builder: SerializationSetupBuilder) -> Result<SerializationSetupBuilder, SerializationBuilderError> {
    let serializer: ArcShared<dyn Serializer> =
      ArcShared::new(StreamRefProtocolSerializer::new(STREAM_REF_PROTOCOL_SERIALIZER_ID));
    let builder = builder.register_serializer(
      STREAM_REF_PROTOCOL_SERIALIZER_NAME,
      STREAM_REF_PROTOCOL_SERIALIZER_ID,
      serializer,
    )?;
    let builder = Self::bind_protocol_types(builder)?;
    Self::register_manifest_routes(builder)
  }

  fn metadata(&self) -> &'static str {
    STREAM_REF_PROTOCOL_SERIALIZER_NAME
  }
}
