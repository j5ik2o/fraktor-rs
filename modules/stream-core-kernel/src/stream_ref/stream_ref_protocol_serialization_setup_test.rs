use alloc::vec;
use core::any::TypeId;

use fraktor_actor_adaptor_std_rs::tick_driver::TestTickDriver;
use fraktor_actor_core_kernel_rs::{
  actor::setup::ActorSystemConfig,
  serialization::{SerializationCallScope, SerializationExtension, SerializationSetupBuilder},
  system::ActorSystem,
};

use super::StreamRefProtocolSerializationSetup;
use crate::stream_ref::{
  ACK_MANIFEST, CUMULATIVE_DEMAND_MANIFEST, ON_SUBSCRIBE_HANDSHAKE_MANIFEST, REMOTE_STREAM_COMPLETED_MANIFEST,
  REMOTE_STREAM_FAILURE_MANIFEST, SEQUENCED_ON_NEXT_MANIFEST, SINK_REF_MANIFEST, SOURCE_REF_MANIFEST,
  STREAM_REF_PROTOCOL_SERIALIZER_ID, STREAM_REF_PROTOCOL_SERIALIZER_NAME as STREAM_REF_SERIALIZER_NAME, StreamRefAck,
  StreamRefCumulativeDemand, StreamRefOnSubscribeHandshake, StreamRefRemoteStreamCompleted,
  StreamRefRemoteStreamFailure, StreamRefSequencedOnNext, StreamRefSinkRefPayload, StreamRefSourceRefPayload,
};

#[test]
fn setup_registers_protocol_bindings_and_manifest_routes() {
  let setup = SerializationSetupBuilder::new()
    .apply_adapter(&StreamRefProtocolSerializationSetup::new())
    .expect("apply stream ref protocol setup")
    .set_fallback(STREAM_REF_SERIALIZER_NAME)
    .expect("set fallback")
    .require_manifest_for_scope(SerializationCallScope::Remote)
    .build()
    .expect("build setup");

  assert_eq!(setup.binding_for(TypeId::of::<StreamRefSequencedOnNext>()), Some(STREAM_REF_PROTOCOL_SERIALIZER_ID));
  assert_eq!(setup.binding_for(TypeId::of::<StreamRefCumulativeDemand>()), Some(STREAM_REF_PROTOCOL_SERIALIZER_ID));
  assert_eq!(setup.binding_for(TypeId::of::<StreamRefRemoteStreamFailure>()), Some(STREAM_REF_PROTOCOL_SERIALIZER_ID));
  assert_eq!(
    setup.binding_for(TypeId::of::<StreamRefRemoteStreamCompleted>()),
    Some(STREAM_REF_PROTOCOL_SERIALIZER_ID),
  );
  assert_eq!(setup.binding_for(TypeId::of::<StreamRefOnSubscribeHandshake>()), Some(STREAM_REF_PROTOCOL_SERIALIZER_ID),);
  assert_eq!(setup.binding_for(TypeId::of::<StreamRefAck>()), Some(STREAM_REF_PROTOCOL_SERIALIZER_ID));
  assert_eq!(setup.binding_for(TypeId::of::<StreamRefSourceRefPayload>()), Some(STREAM_REF_PROTOCOL_SERIALIZER_ID));
  assert_eq!(setup.binding_for(TypeId::of::<StreamRefSinkRefPayload>()), Some(STREAM_REF_PROTOCOL_SERIALIZER_ID));
  assert_eq!(setup.manifest_for(TypeId::of::<StreamRefSequencedOnNext>()), Some(SEQUENCED_ON_NEXT_MANIFEST));
  assert_eq!(setup.manifest_for(TypeId::of::<StreamRefCumulativeDemand>()), Some(CUMULATIVE_DEMAND_MANIFEST));
  assert_eq!(setup.manifest_for(TypeId::of::<StreamRefRemoteStreamFailure>()), Some(REMOTE_STREAM_FAILURE_MANIFEST));
  assert_eq!(
    setup.manifest_for(TypeId::of::<StreamRefRemoteStreamCompleted>()),
    Some(REMOTE_STREAM_COMPLETED_MANIFEST)
  );
  assert_eq!(setup.manifest_for(TypeId::of::<StreamRefOnSubscribeHandshake>()), Some(ON_SUBSCRIBE_HANDSHAKE_MANIFEST));
  assert_eq!(setup.manifest_for(TypeId::of::<StreamRefAck>()), Some(ACK_MANIFEST));
  assert_eq!(setup.manifest_for(TypeId::of::<StreamRefSourceRefPayload>()), Some(SOURCE_REF_MANIFEST));
  assert_eq!(setup.manifest_for(TypeId::of::<StreamRefSinkRefPayload>()), Some(SINK_REF_MANIFEST));
  assert_eq!(
    setup.manifest_routes().get(SEQUENCED_ON_NEXT_MANIFEST),
    Some(&vec![(0, STREAM_REF_PROTOCOL_SERIALIZER_ID)])
  );
  assert_eq!(
    setup.manifest_routes().get(CUMULATIVE_DEMAND_MANIFEST),
    Some(&vec![(0, STREAM_REF_PROTOCOL_SERIALIZER_ID)])
  );
  assert_eq!(
    setup.manifest_routes().get(ON_SUBSCRIBE_HANDSHAKE_MANIFEST),
    Some(&vec![(0, STREAM_REF_PROTOCOL_SERIALIZER_ID)])
  );
  assert_eq!(
    setup.manifest_routes().get(REMOTE_STREAM_COMPLETED_MANIFEST),
    Some(&vec![(0, STREAM_REF_PROTOCOL_SERIALIZER_ID)])
  );
  assert_eq!(
    setup.manifest_routes().get(REMOTE_STREAM_FAILURE_MANIFEST),
    Some(&vec![(0, STREAM_REF_PROTOCOL_SERIALIZER_ID)])
  );
  assert_eq!(setup.manifest_routes().get(ACK_MANIFEST), Some(&vec![(0, STREAM_REF_PROTOCOL_SERIALIZER_ID)]));
  assert_eq!(setup.manifest_routes().get(SOURCE_REF_MANIFEST), Some(&vec![(0, STREAM_REF_PROTOCOL_SERIALIZER_ID)]));
  assert_eq!(setup.manifest_routes().get(SINK_REF_MANIFEST), Some(&vec![(0, STREAM_REF_PROTOCOL_SERIALIZER_ID)]));
}

#[test]
fn registered_extension_serializes_protocol_payload_with_manifest() {
  let setup = SerializationSetupBuilder::new()
    .apply_adapter(&StreamRefProtocolSerializationSetup::new())
    .expect("apply stream ref protocol setup")
    .set_fallback(STREAM_REF_SERIALIZER_NAME)
    .expect("set fallback")
    .require_manifest_for_scope(SerializationCallScope::Remote)
    .build()
    .expect("build setup");
  let config = ActorSystemConfig::new(TestTickDriver::default());
  let system = ActorSystem::create_with_noop_guardian(config).expect("actor system");
  let extension = SerializationExtension::new(&system, setup);

  let serialized = extension.serialize(&StreamRefAck, SerializationCallScope::Remote).expect("serialize ack");
  let decoded = extension.deserialize(&serialized, None).expect("deserialize ack");

  assert_eq!(serialized.serializer_id(), STREAM_REF_PROTOCOL_SERIALIZER_ID);
  assert_eq!(serialized.manifest(), Some(ACK_MANIFEST));
  assert!(decoded.downcast::<StreamRefAck>().is_ok());
}
