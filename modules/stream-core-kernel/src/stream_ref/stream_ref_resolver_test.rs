use alloc::{boxed::Box, vec::Vec};

use fraktor_actor_adaptor_std_rs::system::create_noop_actor_system_with;
use fraktor_actor_core_kernel_rs::{
  actor::{
    actor_ref::ActorRef, actor_ref_provider::LocalActorRefProviderInstaller, messaging::AnyMessage,
    scheduler::SchedulerConfig,
  },
  serialization::{SerializationError, SerializedMessage, SerializerId},
  system::ActorSystem,
};
use fraktor_utils_core_rs::sync::{ArcShared, SpinSyncMutex};

use super::StreamRefResolver;
use crate::{
  StreamError,
  r#impl::streamref::{StreamRefEndpointSlot, StreamRefHandoff},
  stage::{StageActor, StageActorEnvelope, StageActorReceive},
  stream_ref::{
    SINK_REF_MANIFEST, SOURCE_REF_MANIFEST, STREAM_REF_PROTOCOL_SERIALIZER_ID, SinkRef, SourceRef,
    StreamRefSinkRefPayload,
  },
};

struct RecordingReceive {
  values: ArcShared<SpinSyncMutex<Vec<u32>>>,
}

impl RecordingReceive {
  const fn new(values: ArcShared<SpinSyncMutex<Vec<u32>>>) -> Self {
    Self { values }
  }
}

fn recording_stage_actor(system: &ActorSystem) -> (StageActor, ArcShared<SpinSyncMutex<Vec<u32>>>) {
  let values = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  (StageActor::new(system, Box::new(RecordingReceive::new(values.clone()))), values)
}

impl StageActorReceive for RecordingReceive {
  fn receive(&mut self, envelope: StageActorEnvelope) -> Result<(), StreamError> {
    let Some(value) = envelope.message().downcast_ref::<u32>() else {
      return Err(StreamError::TypeMismatch);
    };
    self.values.lock().push(*value);
    Ok(())
  }
}

fn build_system() -> ActorSystem {
  let scheduler = SchedulerConfig::default().with_runner_api_enabled(true);
  create_noop_actor_system_with(|config| {
    config.with_scheduler_config(scheduler).with_actor_ref_provider_installer(LocalActorRefProviderInstaller::default())
  })
}

fn assert_failed_with_context(error: StreamError, message: &str) {
  assert!(matches!(error, StreamError::FailedWithContext { .. }));
  assert!(error.to_string().contains(message));
}

#[test]
fn source_ref_format_round_trip_resolves_same_endpoint_actor_through_provider_dispatch() {
  let system = build_system();
  let (stage_actor, _values) = recording_stage_actor(&system);
  let source_ref = SourceRef::<u32>::from_endpoint_actor(stage_actor.actor_ref().clone());
  let resolver = StreamRefResolver::new(system);

  let serialized = resolver.source_ref_to_format(&source_ref).expect("source ref format");
  let resolved = resolver.resolve_source_ref::<u32>(&serialized).expect("resolved source ref");

  assert_eq!(resolved.endpoint_actor_ref().expect("resolved endpoint actor"), stage_actor.actor_ref().clone());
  assert_eq!(resolved.canonical_actor_path().expect("resolved canonical path"), serialized);
}

#[test]
fn sink_ref_format_round_trip_resolves_same_endpoint_actor_through_provider_dispatch() {
  let system = build_system();
  let (stage_actor, _values) = recording_stage_actor(&system);
  let sink_ref = SinkRef::<u32>::from_endpoint_actor(stage_actor.actor_ref().clone());
  let resolver = StreamRefResolver::new(system);

  let serialized = resolver.sink_ref_to_format(&sink_ref).expect("sink ref format");
  let resolved = resolver.resolve_sink_ref::<u32>(&serialized).expect("resolved sink ref");

  assert_eq!(resolved.endpoint_actor_ref().expect("resolved endpoint actor"), stage_actor.actor_ref().clone());
  assert_eq!(resolved.canonical_actor_path().expect("resolved canonical path"), serialized);
}

#[test]
fn source_ref_serialized_message_round_trip_restores_typed_ref() {
  let system = build_system();
  let (stage_actor, _values) = recording_stage_actor(&system);
  let source_ref = SourceRef::<u32>::from_endpoint_actor(stage_actor.actor_ref().clone());
  let resolver = StreamRefResolver::new(system);

  let serialized = resolver.source_ref_to_serialized_message(&source_ref).expect("source ref serialized message");
  let resolved =
    resolver.resolve_source_ref_message::<u32>(&serialized).expect("resolved source ref serialized message");

  assert_eq!(serialized.serializer_id(), STREAM_REF_PROTOCOL_SERIALIZER_ID);
  assert_eq!(serialized.manifest(), Some(SOURCE_REF_MANIFEST));
  assert_eq!(resolved.endpoint_actor_ref().expect("resolved endpoint actor"), stage_actor.actor_ref().clone());
}

#[test]
fn sink_ref_serialized_message_round_trip_restores_typed_ref() {
  let system = build_system();
  let (stage_actor, _values) = recording_stage_actor(&system);
  let sink_ref = SinkRef::<u32>::from_endpoint_actor(stage_actor.actor_ref().clone());
  let resolver = StreamRefResolver::new(system);

  let serialized = resolver.sink_ref_to_serialized_message(&sink_ref).expect("sink ref serialized message");
  let resolved = resolver.resolve_sink_ref_message::<u32>(&serialized).expect("resolved sink ref serialized message");

  assert_eq!(serialized.serializer_id(), STREAM_REF_PROTOCOL_SERIALIZER_ID);
  assert_eq!(serialized.manifest(), Some(SINK_REF_MANIFEST));
  assert_eq!(resolved.endpoint_actor_ref().expect("resolved endpoint actor"), stage_actor.actor_ref().clone());
}

#[test]
fn resolved_source_ref_endpoint_uses_loopback_actor_delivery() {
  let system = build_system();
  let (stage_actor, values) = recording_stage_actor(&system);
  let source_ref = SourceRef::<u32>::from_endpoint_actor(stage_actor.actor_ref().clone());
  let resolver = StreamRefResolver::new(system);
  let serialized = resolver.source_ref_to_format(&source_ref).expect("source ref format");
  let mut endpoint = resolver
    .resolve_source_ref::<u32>(&serialized)
    .expect("resolved source ref")
    .endpoint_actor_ref()
    .expect("resolved endpoint actor");

  endpoint.try_tell(AnyMessage::new(7_u32)).expect("loopback actor delivery");
  stage_actor.drain_pending().expect("drain loopback delivery");

  assert_eq!(values.lock().as_slice(), &[7_u32]);
}

#[test]
fn resolved_sink_ref_endpoint_uses_loopback_actor_delivery() {
  let system = build_system();
  let (stage_actor, values) = recording_stage_actor(&system);
  let sink_ref = SinkRef::<u32>::from_endpoint_actor(stage_actor.actor_ref().clone());
  let resolver = StreamRefResolver::new(system);
  let serialized = resolver.sink_ref_to_format(&sink_ref).expect("sink ref format");
  let mut endpoint = resolver
    .resolve_sink_ref::<u32>(&serialized)
    .expect("resolved sink ref")
    .endpoint_actor_ref()
    .expect("resolved endpoint actor");

  endpoint.try_tell(AnyMessage::new(11_u32)).expect("loopback actor delivery");
  stage_actor.drain_pending().expect("drain loopback delivery");

  assert_eq!(values.lock().as_slice(), &[11_u32]);
}

#[test]
fn recording_receive_rejects_non_u32_payloads() {
  let values = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let mut receive = RecordingReceive::new(values);

  assert_eq!(
    receive.receive(StageActorEnvelope::new(ActorRef::null(), AnyMessage::new("not u32"))),
    Err(StreamError::TypeMismatch)
  );
}

#[test]
fn source_ref_to_format_rejects_local_ref_without_endpoint_actor() {
  let resolver = StreamRefResolver::new(build_system());
  let source_ref = SourceRef::<u32>::new(StreamRefHandoff::new(), StreamRefEndpointSlot::new());

  let error = resolver.source_ref_to_format(&source_ref).expect_err("local SourceRef must not serialize");

  assert_eq!(error, StreamError::StreamRefTargetNotInitialized);
}

#[test]
fn sink_ref_to_format_rejects_local_ref_without_endpoint_actor() {
  let resolver = StreamRefResolver::new(build_system());
  let sink_ref = SinkRef::<u32>::new(StreamRefHandoff::new(), StreamRefEndpointSlot::new());

  let error = resolver.sink_ref_to_format(&sink_ref).expect_err("local SinkRef must not serialize");

  assert_eq!(error, StreamError::StreamRefTargetNotInitialized);
}

#[test]
fn source_ref_serialized_message_rejects_local_ref_without_endpoint_actor() {
  let resolver = StreamRefResolver::new(build_system());
  let source_ref = SourceRef::<u32>::new(StreamRefHandoff::new(), StreamRefEndpointSlot::new());

  let error =
    resolver.source_ref_to_serialized_message(&source_ref).expect_err("local SourceRef must not serialize as payload");

  assert!(matches!(error, SerializationError::NotSerializable(_)));
}

#[test]
fn sink_ref_serialized_message_rejects_local_ref_without_endpoint_actor() {
  let resolver = StreamRefResolver::new(build_system());
  let sink_ref = SinkRef::<u32>::new(StreamRefHandoff::new(), StreamRefEndpointSlot::new());

  let error =
    resolver.sink_ref_to_serialized_message(&sink_ref).expect_err("local SinkRef must not serialize as payload");

  assert!(matches!(error, SerializationError::NotSerializable(_)));
}

#[test]
fn resolve_sink_ref_message_rejects_source_ref_manifest() {
  let resolver = StreamRefResolver::new(build_system());
  let serialized =
    SerializedMessage::new(STREAM_REF_PROTOCOL_SERIALIZER_ID, Some(SOURCE_REF_MANIFEST.into()), Vec::new());

  let error = resolver
    .resolve_sink_ref_message::<u32>(&serialized)
    .err()
    .expect("SourceRef manifest must not resolve as SinkRef");

  assert_eq!(error, SerializationError::UnknownManifest(SOURCE_REF_MANIFEST.into()));
}

#[test]
fn resolve_source_ref_message_rejects_missing_serializer_registration() {
  let resolver = StreamRefResolver::new(build_system());
  let missing_serializer = SerializerId::from_raw(999);
  let serialized = SerializedMessage::new(missing_serializer, Some(SOURCE_REF_MANIFEST.into()), Vec::new());

  let error = resolver
    .resolve_source_ref_message::<u32>(&serialized)
    .err()
    .expect("missing StreamRef serializer must not resolve SourceRef");

  assert_eq!(error, SerializationError::UnknownSerializer(missing_serializer));
}

#[test]
fn resolve_source_ref_message_rejects_unsupported_manifest() {
  let resolver = StreamRefResolver::new(build_system());
  let serialized =
    SerializedMessage::new(STREAM_REF_PROTOCOL_SERIALIZER_ID, Some("missing.StreamRefManifest".into()), Vec::new());

  let error = resolver
    .resolve_source_ref_message::<u32>(&serialized)
    .err()
    .expect("unsupported manifest must not resolve SourceRef");

  assert_eq!(error, SerializationError::UnknownManifest("missing.StreamRefManifest".into()));
}

#[test]
fn resolve_source_ref_message_rejects_sink_ref_type_mismatch() {
  let resolver = StreamRefResolver::new(build_system());
  let serialized =
    SerializedMessage::new(STREAM_REF_PROTOCOL_SERIALIZER_ID, Some(SINK_REF_MANIFEST.into()), Vec::new());

  let error = resolver
    .resolve_source_ref_message::<u32>(&serialized)
    .err()
    .expect("SinkRef manifest must not resolve as SourceRef");

  assert_eq!(error, SerializationError::UnknownManifest(SINK_REF_MANIFEST.into()));
}

#[test]
fn resolve_source_ref_rejects_invalid_path_format() {
  let resolver = StreamRefResolver::new(build_system());

  let error = resolver.resolve_source_ref::<u32>("not a stream ref path").err().expect("invalid path must fail");

  assert_failed_with_context(error, "invalid StreamRef actor path");
}

#[test]
fn resolve_sink_ref_rejects_missing_endpoint_actor() {
  let resolver = StreamRefResolver::new(build_system());

  let error = resolver
    .resolve_sink_ref::<u32>("fraktor://cellactor/user/temp/missing")
    .err()
    .expect("missing endpoint must fail");

  assert_failed_with_context(error, "StreamRef provider dispatch failed");
}

#[test]
fn serialized_message_payload_validation_rejects_manifest_mismatch_and_missing_manifest() {
  let mismatched_payload = StreamRefSinkRefPayload::new(String::from("fraktor://cellactor/user/temp/ref"));
  let mismatch = StreamRefResolver::payload_to_serialized_message(&mismatched_payload, SOURCE_REF_MANIFEST)
    .expect_err("SinkRef payload must not serialize as SourceRef");
  assert_eq!(mismatch, SerializationError::InvalidFormat);

  let resolver = StreamRefResolver::new(build_system());
  let missing_manifest = SerializedMessage::new(STREAM_REF_PROTOCOL_SERIALIZER_ID, None, Vec::new());
  let error = resolver.resolve_source_ref_message::<u32>(&missing_manifest).err().expect("missing manifest must fail");
  assert_eq!(error, SerializationError::InvalidFormat);
}
