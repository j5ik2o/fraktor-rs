#![cfg(test)]

use alloc::string::String;

use fraktor_actor_rs::core::{
  actor_prim::{Actor, ActorContextGeneric, actor_path::ActorPathParts},
  error::ActorError,
  messaging::{AnyMessageGeneric, SystemMessage},
  props::PropsGeneric,
  serialization::{NullSerializer, SerializationExtensionGeneric, SerializationSetupBuilder, Serializer, SerializerId},
  system::ActorSystemGeneric,
};
use fraktor_utils_rs::core::{runtime_toolbox::NoStdToolbox, sync::ArcShared};

use crate::core::{
  endpoint_manager::RemoteNodeId,
  endpoint_reader::EndpointReader,
  endpoint_writer::{EndpointWriter, OutboundEnvelope},
  transport::{LoopbackTransport, RemoteTransport, TransportBind, TransportEndpoint},
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
  let props = PropsGeneric::from_fn(|| NullActor).with_name("reader-test");
  ActorSystemGeneric::new(&props).expect("actor system")
}

fn build_serialization_extension(
  system: &ActorSystemGeneric<NoStdToolbox>,
) -> ArcShared<SerializationExtensionGeneric<NoStdToolbox>> {
  let serializer_id = SerializerId::try_from(210).expect("id");
  let serializer: ArcShared<dyn Serializer> = ArcShared::new(NullSerializer::new(serializer_id));
  let setup = SerializationSetupBuilder::new()
    .register_serializer("null", serializer_id, serializer)
    .expect("register serializer")
    .set_fallback("null")
    .expect("fallback")
    .bind::<()>("null")
    .expect("bind unit")
    .bind::<SystemMessage>("null")
    .expect("bind system message")
    .bind::<String>("null")
    .expect("bind string")
    .build()
    .expect("setup");
  ArcShared::new(SerializationExtensionGeneric::new(system, setup))
}

#[test]
fn reader_deserializes_payload() {
  let system = build_system();
  let serialization = build_serialization_extension(&system);
  let writer = EndpointWriter::new(serialization.clone());
  let reader = EndpointReader::new(serialization.clone());
  let target = ActorPathParts::with_authority("alpha", Some(("host-a", 6555)));
  let remote = RemoteNodeId::new("beta", "host-b", Some(1777), 5);
  let envelope = writer
    .write(OutboundEnvelope {
      target:  target.clone(),
      remote:  remote.clone(),
      message: AnyMessageGeneric::new("ping".to_string()),
    })
    .expect("serialize");

  let encoded = envelope.encode();
  let inbound = reader.read(&encoded).expect("read");

  assert_eq!(inbound.target().system(), target.system());
  assert_eq!(inbound.remote().uid(), remote.uid());
  assert_eq!(inbound.message().payload().downcast_ref::<String>().expect("string payload"), "ping");
}

#[test]
fn loopback_round_trip_via_transport() {
  use fraktor_utils_rs::core::runtime_toolbox::NoStdToolbox;

  let system = build_system();
  let serialization = build_serialization_extension(&system);
  let mut writer = EndpointWriter::new(serialization.clone());
  let reader = EndpointReader::new(serialization.clone());
  let transport = LoopbackTransport::new();
  let authority = "loop-node";
  let handle =
    <LoopbackTransport as RemoteTransport<NoStdToolbox>>::spawn_listener(&transport, &TransportBind::new(authority))
      .expect("listener");
  let channel =
    <LoopbackTransport as RemoteTransport<NoStdToolbox>>::open_channel(&transport, &TransportEndpoint::new(authority))
      .expect("channel");

  let target = ActorPathParts::with_authority("gamma", Some(("loop", 2001)));
  let remote = RemoteNodeId::new("delta", "loop", Some(2001), 9);

  writer
    .enqueue(OutboundEnvelope {
      target:  target.clone(),
      remote:  remote.clone(),
      message: AnyMessageGeneric::new("hello".to_string()),
    })
    .unwrap_or_else(|_| panic!("enqueue"));

  let outbound = writer.dequeue().unwrap_or_else(|_| panic!("dequeue")).expect("payload available");
  let frame = writer.write(outbound).expect("serialize").encode();
  <LoopbackTransport as RemoteTransport<NoStdToolbox>>::send(&transport, &channel, &frame).expect("send");

  let frames = handle.take_frames();
  assert_eq!(frames.len(), 1);
  let frame = &frames[0];
  let mut len_bytes = [0u8; 4];
  len_bytes.copy_from_slice(&frame[..4]);
  let payload_len = u32::from_be_bytes(len_bytes) as usize;
  let payload = &frame[4..4 + payload_len];
  let inbound = reader.read(payload).expect("decode frame");

  assert_eq!(inbound.remote().system(), remote.system());
  assert_eq!(inbound.message().payload().downcast_ref::<String>().expect("string payload"), "hello");
}
