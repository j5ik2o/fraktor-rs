use crate::core::{
  actor::actor_ref::ActorRef,
  typed::message_adapter::{AdapterEnvelope, AdapterPayload},
};

#[test]
fn envelope_exposes_type_id_and_sender() {
  let payload = AdapterPayload::new(7_u32);
  let sender = ActorRef::null();
  let envelope = AdapterEnvelope::new(payload, Some(sender.clone()));
  assert_eq!(envelope.type_id(), core::any::TypeId::of::<u32>());
  assert!(envelope.sender().is_some());
  let extracted = envelope.take_payload().expect("payload available");
  assert_eq!(extracted.type_id(), core::any::TypeId::of::<u32>());
  assert!(envelope.take_payload().is_none());
}
