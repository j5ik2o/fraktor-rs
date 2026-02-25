use core::{any::Any, ops::Deref};

use fraktor_utils_rs::core::sync::ArcShared;

use crate::core::{
  event_seq::EventSeq, identity_event_adapter::IdentityEventAdapter, read_event_adapter::ReadEventAdapter,
  write_event_adapter::WriteEventAdapter,
};

#[test]
fn identity_event_adapter_preserves_write_payload() {
  let adapter = IdentityEventAdapter::new();
  let payload: ArcShared<dyn Any + Send + Sync> = ArcShared::new(12_i32);

  assert_eq!(adapter.manifest(payload.deref()), "");

  let converted = adapter.to_journal(payload);

  assert_eq!(converted.downcast_ref::<i32>(), Some(&12_i32));
}

#[test]
fn identity_event_adapter_returns_single_event_on_read() {
  let adapter = IdentityEventAdapter::new();
  let payload: ArcShared<dyn Any + Send + Sync> = ArcShared::new(12_i32);

  let sequence = adapter.adapt_from_journal(payload, "ignored");

  match sequence {
    | EventSeq::Single(value) => assert_eq!(value.downcast_ref::<i32>(), Some(&12_i32)),
    | _ => panic!("expected single event"),
  }
}
