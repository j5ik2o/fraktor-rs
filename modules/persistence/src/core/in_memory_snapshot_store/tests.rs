use alloc::boxed::Box;
use core::{
  future::Future,
  task::{Context, Poll, RawWaker, RawWakerVTable, Waker},
};

use fraktor_utils_rs::core::sync::ArcShared;

use crate::core::{
  in_memory_snapshot_store::InMemorySnapshotStore, snapshot_metadata::SnapshotMetadata,
  snapshot_selection_criteria::SnapshotSelectionCriteria, snapshot_store::SnapshotStore,
};

fn noop_waker() -> Waker {
  const VTABLE: RawWakerVTable = RawWakerVTable::new(|data| RawWaker::new(data, &VTABLE), |_| {}, |_| {}, |_| {});

  unsafe fn raw_waker() -> RawWaker {
    RawWaker::new(core::ptr::null(), &VTABLE)
  }

  unsafe { Waker::from_raw(raw_waker()) }
}

fn poll_ready<F: Future>(future: F) -> F::Output {
  let waker = noop_waker();
  let mut cx = Context::from_waker(&waker);
  let mut future = Box::pin(future);
  match Future::poll(future.as_mut(), &mut cx) {
    | Poll::Ready(output) => output,
    | Poll::Pending => panic!("future was pending"),
  }
}

fn payload(value: i32) -> ArcShared<dyn core::any::Any + Send + Sync> {
  ArcShared::new(value)
}

#[test]
fn in_memory_snapshot_store_save_and_load_latest() {
  let mut store = InMemorySnapshotStore::new();
  let metadata1 = SnapshotMetadata::new("pid-1", 1, 10);
  let metadata2 = SnapshotMetadata::new("pid-1", 2, 20);

  poll_ready(store.save_snapshot(metadata1.clone(), payload(1))).expect("save failed");
  poll_ready(store.save_snapshot(metadata2.clone(), payload(2))).expect("save failed");

  let loaded = poll_ready(store.load_snapshot("pid-1", SnapshotSelectionCriteria::latest())).expect("load failed");
  let snapshot = loaded.expect("missing snapshot");
  assert_eq!(snapshot.metadata().sequence_nr(), 2);
}

#[test]
fn in_memory_snapshot_store_load_none() {
  let store = InMemorySnapshotStore::new();

  let loaded = poll_ready(store.load_snapshot("pid-1", SnapshotSelectionCriteria::none())).expect("load failed");
  assert!(loaded.is_none());
}

#[test]
fn in_memory_snapshot_store_delete_snapshot() {
  let mut store = InMemorySnapshotStore::new();
  let metadata1 = SnapshotMetadata::new("pid-1", 1, 10);
  let metadata2 = SnapshotMetadata::new("pid-1", 2, 20);

  poll_ready(store.save_snapshot(metadata1.clone(), payload(1))).expect("save failed");
  poll_ready(store.save_snapshot(metadata2.clone(), payload(2))).expect("save failed");

  poll_ready(store.delete_snapshot(&metadata2)).expect("delete failed");

  let loaded = poll_ready(store.load_snapshot("pid-1", SnapshotSelectionCriteria::latest())).expect("load failed");
  let snapshot = loaded.expect("missing snapshot");
  assert_eq!(snapshot.metadata().sequence_nr(), 1);
}

#[test]
fn in_memory_snapshot_store_delete_snapshots_by_criteria() {
  let mut store = InMemorySnapshotStore::new();
  let metadata1 = SnapshotMetadata::new("pid-1", 1, 10);
  let metadata2 = SnapshotMetadata::new("pid-1", 2, 20);
  let metadata3 = SnapshotMetadata::new("pid-1", 3, 30);

  poll_ready(store.save_snapshot(metadata1.clone(), payload(1))).expect("save failed");
  poll_ready(store.save_snapshot(metadata2.clone(), payload(2))).expect("save failed");
  poll_ready(store.save_snapshot(metadata3.clone(), payload(3))).expect("save failed");

  let criteria = SnapshotSelectionCriteria::new(2, u64::MAX, 0, 0);
  poll_ready(store.delete_snapshots("pid-1", criteria)).expect("delete failed");

  let loaded = poll_ready(store.load_snapshot("pid-1", SnapshotSelectionCriteria::latest())).expect("load failed");
  let snapshot = loaded.expect("missing snapshot");
  assert_eq!(snapshot.metadata().sequence_nr(), 3);
}
