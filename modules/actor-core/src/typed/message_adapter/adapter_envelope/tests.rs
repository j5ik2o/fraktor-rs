use crate::{
  NoStdToolbox,
  actor_prim::actor_ref::ActorRefGeneric,
  typed::message_adapter::{AdapterEnvelope, AdapterPayload},
};

#[test]
fn envelope_exposes_type_id_and_reply_to() {
  let payload = AdapterPayload::<NoStdToolbox>::new(7_u32);
  let reply = ActorRefGeneric::null();
  let envelope = AdapterEnvelope::new(payload, Some(reply.clone()));
  assert_eq!(envelope.type_id(), core::any::TypeId::of::<u32>());
  assert!(envelope.reply_to().is_some());
  let extracted = envelope.take_payload().expect("payload available");
  assert_eq!(extracted.type_id(), core::any::TypeId::of::<u32>());
  assert!(envelope.take_payload().is_none());
}
