use alloc::{boxed::Box, vec::Vec};
use core::{
  any::Any,
  future::Future,
  task::{Context, Poll, Waker},
};

use fraktor_utils_core_rs::sync::ArcShared;

use crate::{
  journal::{InMemoryJournal, Journal, PersistencePluginProxy},
  persistent::PersistentRepr,
  snapshot::{InMemorySnapshotStore, SnapshotMetadata, SnapshotSelectionCriteria, SnapshotStore},
};

fn poll_ready<F: Future>(future: F) -> F::Output {
  let waker = Waker::noop();
  let mut cx = Context::from_waker(waker);
  let mut future = Box::pin(future);
  match Future::poll(future.as_mut(), &mut cx) {
    | Poll::Ready(output) => output,
    | Poll::Pending => panic!("future was pending"),
  }
}

fn build_messages(persistence_id: &str, start: u64, count: u64) -> Vec<PersistentRepr> {
  (0..count)
    .map(|offset| {
      let payload: ArcShared<dyn Any + Send + Sync> = ArcShared::new((start + offset) as i32);
      PersistentRepr::new(persistence_id, start + offset, payload)
    })
    .collect()
}

fn payload(value: i32) -> ArcShared<dyn Any + Send + Sync> {
  ArcShared::new(value)
}

#[test]
fn plugin_proxy_forwards_journal_operations() {
  let mut proxy = PersistencePluginProxy::new(InMemoryJournal::new(), InMemorySnapshotStore::new());
  let messages = build_messages("pid-1", 1, 2);

  poll_ready(Journal::write_messages(&mut proxy, &messages)).expect("write failed");
  let replayed = poll_ready(Journal::replay_messages(&proxy, "pid-1", 1, 10, 10)).expect("replay failed");
  let highest = poll_ready(Journal::highest_sequence_nr(&proxy, "pid-1")).expect("highest failed");

  assert_eq!(replayed.len(), 2);
  assert_eq!(highest, 2);
}

#[test]
fn plugin_proxy_forwards_snapshot_operations() {
  let mut proxy = PersistencePluginProxy::new(InMemoryJournal::new(), InMemorySnapshotStore::new());
  let metadata = SnapshotMetadata::new("pid-1", 1, 10);

  poll_ready(SnapshotStore::save_snapshot(&mut proxy, metadata.clone(), payload(7))).expect("save failed");
  let loaded = poll_ready(SnapshotStore::load_snapshot(&proxy, "pid-1", SnapshotSelectionCriteria::latest()))
    .expect("load failed");
  let snapshot = loaded.expect("snapshot should exist");

  assert_eq!(snapshot.metadata(), &metadata);
}

#[test]
fn plugin_proxy_set_target_replaces_plugins() {
  let mut proxy = PersistencePluginProxy::new(InMemoryJournal::new(), InMemorySnapshotStore::new());
  let messages = build_messages("pid-1", 1, 1);
  poll_ready(Journal::write_messages(&mut proxy, &messages)).expect("write failed");

  proxy.set_target(InMemoryJournal::new(), InMemorySnapshotStore::new());

  let replayed = poll_ready(Journal::replay_messages(&proxy, "pid-1", 1, 10, 10)).expect("replay failed");
  let highest = poll_ready(Journal::highest_sequence_nr(&proxy, "pid-1")).expect("highest failed");
  let loaded = poll_ready(SnapshotStore::load_snapshot(&proxy, "pid-1", SnapshotSelectionCriteria::latest()))
    .expect("load failed");

  assert!(replayed.is_empty());
  assert_eq!(highest, 0);
  assert!(loaded.is_none());
}
