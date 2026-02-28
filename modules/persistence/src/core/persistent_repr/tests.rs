use alloc::string::ToString;
use core::any::{Any, TypeId};

use fraktor_actor_rs::core::actor::Pid;
use fraktor_utils_rs::core::sync::ArcShared;

use crate::core::{
  event_adapters::EventAdapters, event_seq::EventSeq, persistent_repr::PersistentRepr,
  read_event_adapter::ReadEventAdapter, write_event_adapter::WriteEventAdapter,
};

struct AddOneWriteAdapter;

impl WriteEventAdapter for AddOneWriteAdapter {
  fn manifest(&self, _event: &(dyn Any + Send + Sync)) -> String {
    "add-one-v1".into()
  }

  fn to_journal(&self, event: ArcShared<dyn Any + Send + Sync>) -> ArcShared<dyn Any + Send + Sync> {
    let value = event.downcast_ref::<i32>().expect("expected i32 event");
    ArcShared::new(*value + 1)
  }
}

struct IdentityReadAdapter;

impl ReadEventAdapter for IdentityReadAdapter {
  fn adapt_from_journal(&self, event: ArcShared<dyn Any + Send + Sync>, _manifest: &str) -> EventSeq {
    EventSeq::single(event)
  }
}

#[test]
fn persistent_repr_new_and_accessors() {
  let payload: ArcShared<dyn core::any::Any + Send + Sync> = ArcShared::new(7_i32);
  let repr = PersistentRepr::new("pid-1", 3, payload);

  assert_eq!(repr.persistence_id(), "pid-1");
  assert_eq!(repr.sequence_nr(), 3);
  assert_eq!(repr.manifest(), "");
  assert_eq!(repr.writer_uuid(), "");
  assert_eq!(repr.timestamp(), 0);
  assert!(!repr.deleted());
  assert_eq!(repr.sender(), None);
  assert!(repr.metadata().is_none());
  assert!(repr.adapters().is_empty());
  assert_eq!(repr.adapter_type_id(), TypeId::of::<i32>());
  assert_eq!(repr.downcast_ref::<i32>(), Some(&7));
}

#[test]
fn persistent_repr_with_fields() {
  let payload: ArcShared<dyn core::any::Any + Send + Sync> = ArcShared::new(1_i32);
  let metadata: ArcShared<dyn core::any::Any + Send + Sync> = ArcShared::new("meta".to_string());
  let mut adapters = EventAdapters::new();
  let write_adapter: ArcShared<dyn WriteEventAdapter> = ArcShared::new(AddOneWriteAdapter);
  let read_adapter: ArcShared<dyn ReadEventAdapter> = ArcShared::new(IdentityReadAdapter);
  adapters.register::<i32>(write_adapter, read_adapter);
  let repr = PersistentRepr::new("pid-1", 1, payload)
    .with_manifest("manifest-1")
    .with_writer_uuid("writer-1")
    .with_timestamp(99)
    .with_deleted(true)
    .with_sender(Some(Pid::new(1, 2)))
    .with_adapters(adapters)
    .with_metadata(metadata);

  assert_eq!(repr.manifest(), "manifest-1");
  assert_eq!(repr.writer_uuid(), "writer-1");
  assert_eq!(repr.timestamp(), 99);
  assert!(repr.deleted());
  assert_eq!(repr.sender(), Some(Pid::new(1, 2)));
  assert!(repr.metadata().is_some());
  assert_eq!(repr.adapters().len(), 1);
  let converted = repr.adapters().to_journal::<i32>(ArcShared::new(5_i32));
  assert_eq!(converted.downcast_ref::<i32>(), Some(&6_i32));
}
