use alloc::string::{String, ToString};
use core::{any::Any, ops::Deref};

use fraktor_persistence_core_kernel_rs::journal::{ReadEventAdapter, WriteEventAdapter};
use fraktor_utils_core_rs::sync::ArcShared;

use crate::{EventAdapter, EventSeq, event_adapter::KernelEventAdapterBridge};

const SPLIT_MANIFEST: &str = "split";

struct SplitEventAdapter;

impl EventAdapter<u32> for SplitEventAdapter {
  fn manifest(&self, _event: &u32) -> String {
    SPLIT_MANIFEST.to_string()
  }

  fn to_journal(&self, event: u32) -> ArcShared<dyn Any + Send + Sync> {
    ArcShared::new(event + 10)
  }

  fn adapt_from_journal(&self, event: ArcShared<dyn Any + Send + Sync>, manifest: &str) -> EventSeq<u32> {
    let value = event.downcast_ref::<u32>().copied().unwrap_or_default();
    if manifest == SPLIT_MANIFEST {
      return EventSeq::multiple(vec![value, value + 1]);
    }
    EventSeq::single(value)
  }
}

#[test]
fn typed_event_adapter_bridge_preserves_manifest_and_write_payload() {
  let adapter: ArcShared<dyn EventAdapter<u32>> = ArcShared::new(SplitEventAdapter);
  let bridge = KernelEventAdapterBridge::new(adapter);
  let payload: ArcShared<dyn Any + Send + Sync> = ArcShared::new(5_u32);

  assert_eq!(bridge.manifest(payload.deref()), SPLIT_MANIFEST);
  let journal_payload = bridge.to_journal(payload);

  assert_eq!(journal_payload.downcast_ref::<u32>(), Some(&15_u32));
}

#[test]
fn typed_event_adapter_bridge_expands_read_payload() {
  let adapter: ArcShared<dyn EventAdapter<u32>> = ArcShared::new(SplitEventAdapter);
  let bridge = KernelEventAdapterBridge::new(adapter);
  let payload: ArcShared<dyn Any + Send + Sync> = ArcShared::new(5_u32);

  let sequence = bridge.adapt_from_journal(payload, SPLIT_MANIFEST);
  let events = sequence.into_events();

  assert_eq!(events.len(), 2);
  assert_eq!(events[0].downcast_ref::<u32>(), Some(&5_u32));
  assert_eq!(events[1].downcast_ref::<u32>(), Some(&6_u32));
}
