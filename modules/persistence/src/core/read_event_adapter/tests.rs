use core::any::Any;

use fraktor_utils_rs::core::sync::ArcShared;

use crate::core::{event_seq::EventSeq, read_event_adapter::ReadEventAdapter};

const SPLIT_MANIFEST: &str = "split";

struct SplitReadAdapter;

impl ReadEventAdapter for SplitReadAdapter {
  fn adapt_from_journal(&self, event: ArcShared<dyn Any + Send + Sync>, manifest: &str) -> EventSeq {
    if manifest != SPLIT_MANIFEST {
      return EventSeq::single(event);
    }
    let value = event.downcast_ref::<i32>().expect("expected i32 journal payload");
    EventSeq::multiple(vec![ArcShared::new(*value), ArcShared::new(*value + 1)])
  }
}

#[test]
fn read_event_adapter_returns_single_when_manifest_does_not_match() {
  let adapter = SplitReadAdapter;
  let event: ArcShared<dyn Any + Send + Sync> = ArcShared::new(5_i32);

  let sequence = adapter.adapt_from_journal(event, "identity");

  match sequence {
    | EventSeq::Single(value) => assert_eq!(value.downcast_ref::<i32>(), Some(&5_i32)),
    | _ => panic!("expected single event"),
  }
}

#[test]
fn read_event_adapter_expands_payload_when_manifest_matches() {
  let adapter = SplitReadAdapter;
  let event: ArcShared<dyn Any + Send + Sync> = ArcShared::new(5_i32);

  let sequence = adapter.adapt_from_journal(event, SPLIT_MANIFEST);

  match sequence {
    | EventSeq::Multiple(values) => {
      assert_eq!(values.len(), 2);
      assert_eq!(values[0].downcast_ref::<i32>(), Some(&5_i32));
      assert_eq!(values[1].downcast_ref::<i32>(), Some(&6_i32));
    },
    | _ => panic!("expected expanded event sequence"),
  }
}
