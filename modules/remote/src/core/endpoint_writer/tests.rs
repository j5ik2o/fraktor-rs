use alloc::string::String;

use fraktor_actor_rs::core::{
  actor_prim::{
    Actor, ActorContextGeneric,
    actor_path::{ActorPath, ActorPathParts, GuardianKind},
  },
  error::ActorError,
  messaging::AnyMessageGeneric,
  props::PropsGeneric,
  scheduler::{ManualTestDriver, TickDriverConfig},
  serialization::{
    SerializationCallScope, SerializationExtensionGeneric, SerializationSetup, SerializationSetupBuilder, Serializer,
    SerializerId, StringSerializer,
  },
  system::{ActorSystemBuilder, ActorSystemGeneric},
};
use fraktor_utils_rs::core::{runtime_toolbox::NoStdToolbox, sync::ArcShared};

use super::*;
use crate::core::{
  outbound_message::OutboundMessage, outbound_priority::OutboundPriority, remote_node_id::RemoteNodeId,
};

struct NoopActor;

impl Actor<NoStdToolbox> for NoopActor {
  fn receive(
    &mut self,
    _ctx: &mut ActorContextGeneric<'_, NoStdToolbox>,
    _message: fraktor_actor_rs::core::messaging::AnyMessageViewGeneric<'_, NoStdToolbox>,
  ) -> Result<(), ActorError> {
    Ok(())
  }
}

fn build_system() -> ActorSystemGeneric<NoStdToolbox> {
  let props = PropsGeneric::from_fn(|| NoopActor).with_name("writer-tests");
  ActorSystemBuilder::new(props)
    .with_tick_driver(TickDriverConfig::manual(ManualTestDriver::new()))
    .build()
    .expect("system builds")
}

fn serialization_setup() -> SerializationSetup {
  let serializer_id = SerializerId::try_from(50).expect("id");
  let serializer: ArcShared<dyn Serializer> = ArcShared::new(StringSerializer::new(serializer_id));
  SerializationSetupBuilder::new()
    .register_serializer("string", serializer_id, serializer)
    .expect("register")
    .bind::<String>("string")
    .expect("bind string")
    .bind_remote_manifest::<String>("tests.String")
    .expect("manifest")
    .set_fallback("string")
    .expect("fallback")
    .require_manifest_for_scope(SerializationCallScope::Remote)
    .build()
    .expect("setup")
}

fn build_writer() -> (EndpointWriter<NoStdToolbox>, ActorSystemGeneric<NoStdToolbox>) {
  let system = build_system();
  let setup = serialization_setup();
  let serialization = ArcShared::new(SerializationExtensionGeneric::new(&system, setup));
  (EndpointWriter::new(system.clone(), serialization), system)
}

fn remote_node() -> RemoteNodeId {
  RemoteNodeId::new("remote-system", "127.0.0.1", Some(2552), 42)
}

fn actor_path(system: &str, guardian: GuardianKind, segments: &[&str]) -> ActorPath {
  let mut path = ActorPath::from_parts(ActorPathParts::local(system).with_guardian(guardian));
  for segment in segments {
    path = path.child(segment);
  }
  path
}

fn user_message(content: &str, recipient: &ActorPath, reply_to: Option<ActorPath>) -> OutboundMessage<NoStdToolbox> {
  let message = AnyMessageGeneric::new(content.to_string());
  let remote = remote_node();
  match reply_to {
    | Some(path) => OutboundMessage::user(message, recipient.clone(), remote).with_reply_to(path),
    | None => OutboundMessage::user(message, recipient.clone(), remote),
  }
}

fn system_message(content: &str, recipient: &ActorPath) -> OutboundMessage<NoStdToolbox> {
  let message = AnyMessageGeneric::new(content.to_string());
  OutboundMessage::system(message, recipient.clone(), remote_node())
}

#[test]
fn serialize_user_message_includes_manifest_and_reply_to() {
  let (writer, _system) = build_writer();
  let recipient = actor_path("remote-app", GuardianKind::User, &["user", "service"]);
  let reply_to = actor_path("local-app", GuardianKind::User, &["user", "client"]);
  writer.enqueue(user_message("ping", &recipient, Some(reply_to.clone()))).expect("enqueue user");

  let envelope = writer.try_next().expect("poll success").expect("envelope present");
  assert_eq!(envelope.priority(), OutboundPriority::User);
  assert_eq!(envelope.recipient(), &recipient);
  assert_eq!(envelope.reply_to(), Some(&reply_to));
  assert_eq!(envelope.remote_node(), &remote_node());
  let serialized = envelope.serialized_message();
  assert_eq!(serialized.manifest(), Some("tests.String"));
  assert!(serialized.bytes().ends_with(b"ping"));
}

#[test]
fn system_priority_and_backpressure_control() {
  let (writer, _system) = build_writer();
  let recipient = actor_path("remote-app", GuardianKind::User, &["user", "service"]);
  writer.enqueue(user_message("user-1", &recipient, None)).expect("enqueue user");
  writer.enqueue(system_message("sys-1", &recipient)).expect("enqueue system");

  let first = writer.try_next().expect("poll").expect("envelope");
  assert!(first.is_system());

  writer.handle_backpressure(BackpressureSignal::Apply);
  writer.enqueue(user_message("user-2", &recipient, None)).expect("enqueue user");
  let blocked = writer.try_next().expect("poll");
  assert!(blocked.is_none());

  writer.enqueue(system_message("sys-2", &recipient)).expect("enqueue system");
  let sys_delivery = writer.try_next().expect("poll").expect("envelope");
  assert!(sys_delivery.is_system());

  writer.handle_backpressure(BackpressureSignal::Release);
  let resumed = writer.try_next().expect("poll").expect("envelope");
  assert_eq!(resumed.priority(), OutboundPriority::User);
}
