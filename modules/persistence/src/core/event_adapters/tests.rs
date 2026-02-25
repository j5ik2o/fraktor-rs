use core::any::Any;

use fraktor_utils_rs::core::sync::ArcShared;

use crate::core::{
  event_adapters::EventAdapters, event_seq::EventSeq, read_event_adapter::ReadEventAdapter,
  write_event_adapter::WriteEventAdapter,
};

const SPLIT_MANIFEST: &str = "split";

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

struct MultiplyWriteAdapter;

impl WriteEventAdapter for MultiplyWriteAdapter {
  fn manifest(&self, _event: &(dyn Any + Send + Sync)) -> String {
    "multiply-v1".into()
  }

  fn to_journal(&self, event: ArcShared<dyn Any + Send + Sync>) -> ArcShared<dyn Any + Send + Sync> {
    let value = event.downcast_ref::<i32>().expect("expected i32 event");
    ArcShared::new(*value * 2)
  }
}

struct SplitReadAdapter;

impl ReadEventAdapter for SplitReadAdapter {
  fn adapt_from_journal(&self, event: ArcShared<dyn Any + Send + Sync>, manifest: &str) -> EventSeq {
    if manifest != SPLIT_MANIFEST {
      return EventSeq::single(event);
    }
    let value = event.downcast_ref::<i32>().expect("expected i32 event");
    EventSeq::multiple(vec![ArcShared::new(*value), ArcShared::new(*value + 1)])
  }
}

#[test]
fn event_adapters_resolve_registered_adapters_by_type() {
  let mut adapters = EventAdapters::new();
  let write_adapter: ArcShared<dyn WriteEventAdapter> = ArcShared::new(AddOneWriteAdapter);
  let read_adapter: ArcShared<dyn ReadEventAdapter> = ArcShared::new(SplitReadAdapter);
  adapters.register::<i32>(write_adapter, read_adapter);

  assert_eq!(adapters.len(), 1);

  let converted = adapters.to_journal::<i32>(ArcShared::new(10_i32));
  assert_eq!(converted.downcast_ref::<i32>(), Some(&11_i32));

  let sequence = adapters.adapt_from_journal::<i32>(ArcShared::new(3_i32), SPLIT_MANIFEST);
  match sequence {
    | EventSeq::Multiple(values) => {
      assert_eq!(values.len(), 2);
      assert_eq!(values[0].downcast_ref::<i32>(), Some(&3_i32));
      assert_eq!(values[1].downcast_ref::<i32>(), Some(&4_i32));
    },
    | _ => panic!("expected expanded replay events"),
  }
}

#[test]
fn event_adapters_fallback_to_identity_for_unregistered_type() {
  let adapters = EventAdapters::new();

  let converted = adapters.to_journal::<u64>(ArcShared::new(8_u64));
  assert_eq!(converted.downcast_ref::<u64>(), Some(&8_u64));

  let sequence = adapters.adapt_from_journal::<u64>(ArcShared::new(8_u64), SPLIT_MANIFEST);
  match sequence {
    | EventSeq::Single(value) => assert_eq!(value.downcast_ref::<u64>(), Some(&8_u64)),
    | _ => panic!("expected identity replay result"),
  }
}

#[test]
fn event_adapters_register_replaces_existing_binding() {
  let mut adapters = EventAdapters::new();
  let first_write: ArcShared<dyn WriteEventAdapter> = ArcShared::new(AddOneWriteAdapter);
  let second_write: ArcShared<dyn WriteEventAdapter> = ArcShared::new(MultiplyWriteAdapter);
  let read_adapter: ArcShared<dyn ReadEventAdapter> = ArcShared::new(SplitReadAdapter);
  adapters.register::<i32>(first_write, read_adapter.clone());
  adapters.register::<i32>(second_write, read_adapter);

  assert_eq!(adapters.len(), 1);
  let converted = adapters.to_journal::<i32>(ArcShared::new(7_i32));
  assert_eq!(converted.downcast_ref::<i32>(), Some(&14_i32));
}
