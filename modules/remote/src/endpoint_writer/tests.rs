#![cfg(test)]

use fraktor_actor_rs::core::{
  actor_prim::{Actor, ActorContextGeneric, actor_path::ActorPathParts},
  error::ActorError,
  event_stream::BackpressureSignal,
  messaging::{AnyMessageGeneric, SystemMessage},
  props::PropsGeneric,
  serialization::{NullSerializer, SerializationExtensionGeneric, SerializationSetupBuilder, Serializer, SerializerId},
  system::ActorSystemGeneric,
};
use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdToolbox, RuntimeToolbox},
  sync::ArcShared,
};

use crate::{
  endpoint_manager::RemoteNodeId,
  endpoint_writer::{EndpointWriter, OutboundEnvelope},
};

struct NullActor;

impl Actor for NullActor {
  fn receive(
    &mut self,
    _ctx: &mut ActorContextGeneric<'_, NoStdToolbox>,
    _message: fraktor_actor_rs::core::messaging::AnyMessageViewGeneric<'_, NoStdToolbox>,
  ) -> Result<(), ActorError> {
    Ok(())
  }
}

fn build_system() -> ActorSystemGeneric<NoStdToolbox> {
  let props = PropsGeneric::from_fn(|| NullActor).with_name("writer-test");
  ActorSystemGeneric::new(&props).expect("actor system")
}

fn build_serialization_extension(
  system: &ActorSystemGeneric<NoStdToolbox>,
) -> ArcShared<SerializationExtensionGeneric<NoStdToolbox>> {
  let serializer_id = SerializerId::try_from(200).expect("id");
  let serializer: ArcShared<dyn Serializer> = ArcShared::new(NullSerializer::new(serializer_id));
  let setup = SerializationSetupBuilder::new()
    .register_serializer("null", serializer_id, serializer)
    .expect("register serializer")
    .set_fallback("null")
    .expect("fallback")
    .bind::<()>("null")
    .expect("bind unit")
    .build()
    .expect("setup");
  ArcShared::new(SerializationExtensionGeneric::new(system, setup))
}

#[test]
fn writer_serializes_payload_and_reply_metadata() {
  let system = build_system();
  let serialization = build_serialization_extension(&system);
  let writer = EndpointWriter::new(serialization.clone());

  let mut message = AnyMessageGeneric::new(());
  let reply_ref = system.user_guardian_ref();
  message = message.with_reply_to(reply_ref.clone());

  let target = ActorPathParts::with_authority("cluster", Some(("remote-host", 2552)));
  let remote = RemoteNodeId::new("cluster", "remote-host", Some(2552), 99);

  let envelope =
    writer.write(OutboundEnvelope { target: target.clone(), remote: remote.clone(), message }).expect("write");
  assert_eq!(envelope.target().system(), target.system());
  assert_eq!(envelope.remote().uid(), remote.uid());
  assert_eq!(envelope.payload().bytes(), &[]);
  let reply_path = envelope.reply_to().expect("reply path");
  assert_eq!(reply_path.system(), reply_ref.path().unwrap().parts().system());
}

fn system_envelope<TB: RuntimeToolbox + 'static>(remote: RemoteNodeId, target: ActorPathParts) -> OutboundEnvelope<TB> {
  let message = AnyMessageGeneric::new(SystemMessage::Stop);
  OutboundEnvelope { target, remote, message }
}

fn user_envelope<TB: RuntimeToolbox + 'static>(remote: RemoteNodeId, target: ActorPathParts) -> OutboundEnvelope<TB> {
  let message = AnyMessageGeneric::new(());
  OutboundEnvelope { target, remote, message }
}

#[test]
fn writer_prioritizes_system_envelopes() {
  let system = build_system();
  let serialization = build_serialization_extension(&system);
  let mut writer = EndpointWriter::new(serialization.clone());
  let target = ActorPathParts::with_authority("cluster", Some(("remote", 2552)));
  let remote = RemoteNodeId::new("cluster", "remote", Some(2552), 1);

  writer.enqueue(user_envelope(remote.clone(), target.clone())).unwrap_or_else(|_| panic!("enqueue user"));
  writer.enqueue(system_envelope(remote.clone(), target.clone())).unwrap_or_else(|_| panic!("enqueue system"));

  let first = writer.dequeue().unwrap_or_else(|_| panic!("dequeue result")).expect("first");
  assert!(first.message.payload().is::<SystemMessage>());
  let second = writer.dequeue().unwrap_or_else(|_| panic!("dequeue result")).expect("second");
  assert!(!second.message.payload().is::<SystemMessage>());
}

#[test]
fn writer_pauses_user_queue_during_backpressure() {
  let system = build_system();
  let serialization = build_serialization_extension(&system);
  let mut writer = EndpointWriter::new(serialization.clone());
  let target = ActorPathParts::with_authority("cluster", Some(("remote", 2552)));
  let remote = RemoteNodeId::new("cluster", "remote", Some(2552), 1);

  writer.enqueue(user_envelope(remote.clone(), target.clone())).unwrap_or_else(|_| panic!("enqueue user"));
  writer.enqueue(system_envelope(remote.clone(), target.clone())).unwrap_or_else(|_| panic!("enqueue system"));

  writer.notify_backpressure(BackpressureSignal::Apply);
  let first = writer.dequeue().unwrap_or_else(|_| panic!("dequeue result")).expect("system available");
  assert!(first.message.payload().is::<SystemMessage>());
  assert!(writer.dequeue().unwrap_or_else(|_| panic!("paused dequeue")).is_none());

  writer.notify_backpressure(BackpressureSignal::Release);
  let resumed = writer.dequeue().unwrap_or_else(|_| panic!("dequeue result")).expect("user resumed");
  assert!(!resumed.message.payload().is::<SystemMessage>());
}
