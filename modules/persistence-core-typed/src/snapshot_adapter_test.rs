use alloc::string::{String, ToString};
use core::any::Any;

use fraktor_utils_core_rs::sync::ArcShared;

use crate::SnapshotAdapter;

const SNAPSHOT_MANIFEST: &str = "counter";

struct CounterSnapshotAdapter;

impl SnapshotAdapter<u32> for CounterSnapshotAdapter {
  fn manifest(&self, _state: &u32) -> String {
    SNAPSHOT_MANIFEST.to_string()
  }

  fn to_snapshot(&self, state: u32) -> ArcShared<dyn Any + Send + Sync> {
    ArcShared::new(state.to_string())
  }

  fn adapt_from_snapshot(&self, snapshot: ArcShared<dyn Any + Send + Sync>, manifest: &str) -> Option<u32> {
    if manifest != SNAPSHOT_MANIFEST {
      return None;
    }
    snapshot.downcast_ref::<String>().and_then(|value| value.parse::<u32>().ok())
  }
}

#[test]
fn typed_snapshot_adapter_round_trips_state() {
  let adapter = CounterSnapshotAdapter;

  let manifest = adapter.manifest(&42_u32);
  let snapshot = adapter.to_snapshot(42_u32);
  let restored = adapter.adapt_from_snapshot(snapshot, &manifest);

  assert_eq!(restored, Some(42_u32));
}
