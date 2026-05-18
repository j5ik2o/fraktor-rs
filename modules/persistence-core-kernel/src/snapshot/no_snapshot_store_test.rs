use core::{
  future::{Future, pending},
  task::{Context, Poll, Waker},
};
use std::panic::catch_unwind;

use fraktor_utils_core_rs::sync::ArcShared;

use crate::snapshot::{NoSnapshotStore, SnapshotMetadata, SnapshotSelectionCriteria, SnapshotStore};

fn poll_ready<F: Future>(future: F) -> F::Output {
  let waker = Waker::noop();
  let mut context = Context::from_waker(waker);
  let mut future = Box::pin(future);
  match Future::poll(future.as_mut(), &mut context) {
    | Poll::Ready(output) => output,
    | Poll::Pending => panic!("future was pending"),
  }
}

#[test]
fn no_snapshot_store_ignores_writes_and_returns_no_snapshot() {
  let mut store = NoSnapshotStore::new();
  let metadata = SnapshotMetadata::new("pid-1", 1, 10);

  poll_ready(store.save_snapshot(metadata.clone(), ArcShared::new(1_i32))).expect("save snapshot");
  let loaded = poll_ready(store.load_snapshot("pid-1", SnapshotSelectionCriteria::latest())).expect("load snapshot");
  poll_ready(store.delete_snapshot(&metadata)).expect("delete snapshot");
  poll_ready(store.delete_snapshots("pid-1", SnapshotSelectionCriteria::latest())).expect("delete snapshots");

  assert!(loaded.is_none());
}

#[test]
fn poll_ready_panics_when_future_is_pending() {
  let result = catch_unwind(|| poll_ready(pending::<()>()));

  assert!(result.is_err());
}
