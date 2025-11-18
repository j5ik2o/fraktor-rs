#![cfg(test)]

use fraktor_actor_rs::core::{
  actor_prim::{Actor, ActorContextGeneric, actor_path::ActorPathParts},
  error::ActorError,
  messaging::AnyMessageGeneric,
  props::PropsGeneric,
  serialization::{
    NullSerializer,
    SerializationExtensionGeneric,
    SerializationSetupBuilder,
    Serializer,
    SerializerId,
  },
  system::ActorSystemGeneric,
};
use fraktor_utils_rs::core::{runtime_toolbox::NoStdToolbox, sync::ArcShared};

use crate::{endpoint_writer::{EndpointWriter, OutboundEnvelope}, RemoteNodeId};

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

fn build_serialization_extension(system: &ActorSystemGeneric<NoStdToolbox>) -> ArcShared<SerializationExtensionGeneric<NoStdToolbox>> {
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

  let envelope = writer.write(OutboundEnvelope { target: target.clone(), remote: remote.clone(), message }).expect("write");
  assert_eq!(envelope.target().system(), target.system());
  assert_eq!(envelope.remote().uid(), remote.uid());
  assert_eq!(envelope.payload().bytes(), &[]);
  let reply_path = envelope.reply_to().expect("reply path");
  assert_eq!(reply_path.system(), reply_ref.path().unwrap().parts().system());
}
