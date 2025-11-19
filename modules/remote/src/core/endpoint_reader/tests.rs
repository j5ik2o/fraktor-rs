#![cfg(any(test, feature = "test-support"))]

use alloc::string::String;

use fraktor_actor_rs::core::{
  actor_prim::{
    Actor, ActorContextGeneric,
    actor_path::{ActorPath, ActorPathParts, GuardianKind},
  },
  error::ActorError,
  messaging::{AnyMessageGeneric, AnyMessageViewGeneric},
  props::PropsGeneric,
  scheduler::{ManualTestDriver, TickDriverConfig},
  serialization::{
    SerializationCallScope, SerializationExtensionGeneric, SerializationSetup, SerializationSetupBuilder,
    SerializedMessage, Serializer, SerializerId, StringSerializer,
  },
  system::{ActorSystemConfig, ActorSystemGeneric},
};
use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdMutex, NoStdToolbox},
  sync::ArcShared,
};

use super::EndpointReader;
use crate::core::{
  inbound_envelope::InboundEnvelope, outbound_message::OutboundMessage, remote_node_id::RemoteNodeId,
  remoting_envelope::RemotingEnvelope,
};

struct NoopActor;

impl Actor<NoStdToolbox> for NoopActor {
  fn receive(
    &mut self,
    _ctx: &mut ActorContextGeneric<'_, NoStdToolbox>,
    _message: AnyMessageViewGeneric<'_, NoStdToolbox>,
  ) -> Result<(), ActorError> {
    Ok(())
  }
}

struct RecordingActor {
  events: ArcShared<NoStdMutex<Vec<String>>>,
}

impl Actor<NoStdToolbox> for RecordingActor {
  fn receive(
    &mut self,
    _ctx: &mut ActorContextGeneric<'_, NoStdToolbox>,
    message: AnyMessageViewGeneric<'_, NoStdToolbox>,
  ) -> Result<(), ActorError> {
    if let Some(payload) = message.downcast_ref::<String>() {
      self.events.lock().push(payload.clone());
    }
    Ok(())
  }
}

fn build_system() -> ActorSystemGeneric<NoStdToolbox> {
  let props = PropsGeneric::from_fn(|| NoopActor).with_name("reader-tests");
  let system_config = ActorSystemConfig::default().with_tick_driver(TickDriverConfig::manual(ManualTestDriver::new()));
  ActorSystemGeneric::new_with_config(&props, &system_config).expect("system builds")
}

fn serialization_setup() -> SerializationSetup {
  let serializer_id = SerializerId::try_from(80).expect("serializer id");
  let serializer: ArcShared<dyn Serializer> = ArcShared::new(StringSerializer::new(serializer_id));
  SerializationSetupBuilder::new()
    .register_serializer("string", serializer_id, serializer)
    .expect("register serializer")
    .bind::<String>("string")
    .expect("bind string")
    .bind_remote_manifest::<String>("tests.String")
    .expect("manifest binding")
    .set_fallback("string")
    .expect("fallback")
    .require_manifest_for_scope(SerializationCallScope::Remote)
    .build()
    .expect("build setup")
}

fn serialization_extension(
  system: &ActorSystemGeneric<NoStdToolbox>,
) -> ArcShared<SerializationExtensionGeneric<NoStdToolbox>> {
  ArcShared::new(SerializationExtensionGeneric::new(system, serialization_setup()))
}

fn remote_node() -> RemoteNodeId {
  RemoteNodeId::new("remote-system", "127.0.0.1", Some(4100), 777)
}

fn recipient_path(system: &str, guardian: GuardianKind, segments: &[&str]) -> ActorPath {
  let mut path = ActorPath::from_parts(ActorPathParts::local(system).with_guardian(guardian));
  for segment in segments {
    path = path.child(segment);
  }
  path
}

fn outbound_message(recipient: &ActorPath) -> OutboundMessage<NoStdToolbox> {
  let message = AnyMessageGeneric::new("ping".to_string());
  OutboundMessage::user(message, recipient.clone(), remote_node())
}

#[test]
fn decode_round_trip_returns_inbound_envelope() {
  let system = build_system();
  let serialization = serialization_extension(&system);
  let reader = EndpointReader::new(system.clone(), serialization.clone());
  let recipient = recipient_path("remote-app", GuardianKind::User, &["user", "svc"]);
  let outbound = outbound_message(&recipient);
  let writer = crate::core::EndpointWriter::new(system.clone(), serialization.clone());
  writer.enqueue(outbound).expect("enqueue");
  let remoting_envelope = writer.try_next().expect("serialize").expect("envelope");

  let inbound = reader.decode(remoting_envelope).expect("decode succeeds");

  assert_eq!(inbound.recipient().to_relative_string(), "/user/user/svc");
  assert_eq!(inbound.remote_node().system(), "remote-system");
  assert!(matches!(inbound.message().as_view().downcast_ref::<String>(), Some(payload) if payload == "ping"));
  assert!(inbound.reply_to_path().is_none());
}

#[test]
fn deserialization_failure_produces_dead_letter_error() {
  let system = build_system();
  let serialization = serialization_extension(&system);
  let reader = EndpointReader::new(system.clone(), serialization.clone());
  let recipient = recipient_path("remote-app", GuardianKind::User, &["user", "svc"]);
  let serialized = SerializedMessage::new(SerializerId::try_from(99).expect("id"), None, vec![1, 2, 3]);
  let envelope = RemotingEnvelope::new(
    recipient.clone(),
    remote_node(),
    None,
    serialized,
    fraktor_actor_rs::core::event_stream::CorrelationId::from_u128(1),
    crate::core::OutboundPriority::User,
  );

  let result: Result<InboundEnvelope<_>, _> = reader.decode(envelope);
  assert!(result.is_err());
  assert!(
    system
      .dead_letters()
      .iter()
      .any(|entry| entry.reason() == fraktor_actor_rs::core::dead_letter::DeadLetterReason::SerializationError)
  );
}

#[test]
fn deliver_routes_message_to_local_actor() {
  let system = build_system();
  let serialization = serialization_extension(&system);
  let reader = EndpointReader::new(system.clone(), serialization);
  let events: ArcShared<NoStdMutex<Vec<String>>> = ArcShared::new(NoStdMutex::new(Vec::new()));
  let props = PropsGeneric::from_fn({
    let events = events.clone();
    move || RecordingActor { events: events.clone() }
  })
  .with_name("recorder");
  let child = system.extended().spawn_system_actor(&props).expect("spawn");
  let actor_ref = child.actor_ref().clone();
  let recipient = actor_ref.path().expect("recipient path");

  let inbound = InboundEnvelope::new(
    recipient,
    remote_node(),
    AnyMessageGeneric::new("delivered".to_string()),
    None,
    fraktor_actor_rs::core::event_stream::CorrelationId::from_u128(1),
    crate::core::OutboundPriority::User,
  );

  reader.deliver(inbound).expect("deliver succeeds");
  assert_eq!(events.lock().as_slice(), &["delivered".to_string()]);
}
