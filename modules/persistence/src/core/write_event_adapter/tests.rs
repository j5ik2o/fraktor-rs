use core::{any::Any, ops::Deref};

use fraktor_utils_rs::core::sync::ArcShared;

use crate::core::write_event_adapter::WriteEventAdapter;

const INCREMENT_MANIFEST: &str = "increment-v1";

struct IncrementWriteAdapter;

impl WriteEventAdapter for IncrementWriteAdapter {
  fn manifest(&self, _event: &(dyn Any + Send + Sync)) -> String {
    INCREMENT_MANIFEST.into()
  }

  fn to_journal(&self, event: ArcShared<dyn Any + Send + Sync>) -> ArcShared<dyn Any + Send + Sync> {
    let value = event.downcast_ref::<i32>().expect("expected i32 event");
    ArcShared::new(*value + 1)
  }
}

#[test]
fn write_event_adapter_converts_payload() {
  let adapter = IncrementWriteAdapter;
  let event: ArcShared<dyn Any + Send + Sync> = ArcShared::new(10_i32);

  assert_eq!(adapter.manifest(event.deref()), INCREMENT_MANIFEST);

  let adapted = adapter.to_journal(event);

  assert_eq!(adapted.downcast_ref::<i32>(), Some(&11_i32));
}
