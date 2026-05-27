extern crate std;

use core::{
  any::Any,
  future::Future,
  task::{Context, Poll, Waker},
};
use std::{
  fs,
  io::ErrorKind,
  path::{Path, PathBuf},
  time::{SystemTime, UNIX_EPOCH},
};

use fraktor_actor_core_kernel_rs::serialization::{
  builtin, default_serialization_setup, serialization_registry::SerializationRegistry,
};
use fraktor_persistence_core_kernel_rs::{
  serialization::register_persistence_serializers,
  snapshot::{Snapshot, SnapshotMetadata, SnapshotSelectionCriteria, SnapshotStore},
};
use fraktor_utils_core_rs::sync::ArcShared;

use super::LocalSnapshotStore;
use crate::snapshot::LocalSnapshotStoreConfig;

fn poll_ready<F: Future>(future: F) -> F::Output {
  let waker = Waker::noop();
  let mut cx = Context::from_waker(waker);
  let mut future = Box::pin(future);
  match Future::poll(future.as_mut(), &mut cx) {
    | Poll::Ready(output) => output,
    | Poll::Pending => panic!("local snapshot store future should be ready"),
  }
}

fn serialization_registry() -> ArcShared<SerializationRegistry> {
  let setup = default_serialization_setup();
  let registry = ArcShared::new(SerializationRegistry::from_setup(&setup));
  builtin::register_defaults(&registry, |name, id| panic!("unexpected serializer collision for {name}: {id:?}"))
    .expect("register builtin serializers");
  register_persistence_serializers(&registry).expect("register persistence serializers");
  registry
}

fn payload(value: i32) -> ArcShared<dyn Any + Send + Sync> {
  ArcShared::new(value)
}

fn unique_snapshot_dir(name: &str) -> PathBuf {
  let timestamp = match SystemTime::now().duration_since(UNIX_EPOCH) {
    | Ok(duration) => duration.as_nanos(),
    | Err(error) => panic!("system clock should be after unix epoch: {error}"),
  };
  std::env::temp_dir().join(format!("fraktor-local-snapshot-store-{name}-{}-{timestamp}", std::process::id()))
}

fn remove_dir_if_exists(path: &Path) {
  match fs::remove_dir_all(path) {
    | Ok(()) => (),
    | Err(error) if error.kind() == ErrorKind::NotFound => (),
    | Err(error) => panic!("snapshot test directory should be removable: {error}"),
  }
}

fn open_store(directory: &Path, max_load_attempts: usize) -> LocalSnapshotStore {
  let config = LocalSnapshotStoreConfig::new(directory.to_path_buf(), serialization_registry())
    .with_max_load_attempts(max_load_attempts);
  LocalSnapshotStore::open(config).expect("open local snapshot store")
}

fn save_snapshot(store: &mut LocalSnapshotStore, metadata: SnapshotMetadata, value: i32) {
  poll_ready(store.save_snapshot(metadata, payload(value))).expect("save snapshot");
}

fn load_latest(store: &LocalSnapshotStore, persistence_id: &str) -> Option<Snapshot> {
  poll_ready(store.load_snapshot(persistence_id, SnapshotSelectionCriteria::latest())).expect("load latest snapshot")
}

fn assert_snapshot(snapshot: &Snapshot, sequence_nr: u64, value: i32) {
  assert_eq!(snapshot.metadata().sequence_nr(), sequence_nr);
  assert_eq!(snapshot.downcast_ref::<i32>(), Some(&value));
}

fn snapshot_file_for_sequence(directory: &Path, sequence_nr: u64) -> PathBuf {
  let sequence_marker = format!("-{sequence_nr}-");
  let entries = fs::read_dir(directory).expect("read snapshot directory");
  for entry_result in entries {
    let entry = entry_result.expect("read snapshot directory entry");
    let file_name = entry.file_name();
    let file_name = file_name.to_string_lossy();
    if file_name.starts_with("snapshot-") && file_name.contains(&sequence_marker) {
      return entry.path();
    }
  }
  panic!("snapshot file for sequence {sequence_nr} should exist");
}

#[test]
fn local_snapshot_store_open_creates_directory() {
  let directory = unique_snapshot_dir("open");
  remove_dir_if_exists(&directory);

  let _store = open_store(&directory, 3);

  assert!(directory.is_dir());
  remove_dir_if_exists(&directory);
}

#[test]
fn local_snapshot_store_clone_uses_same_directory() {
  let directory = unique_snapshot_dir("clone");
  let store = open_store(&directory, 3);
  let mut cloned = store.clone();

  save_snapshot(&mut cloned, SnapshotMetadata::new("pid-1", 1, 10), 1);
  let loaded = load_latest(&store, "pid-1").expect("snapshot should be loaded through original clone source");

  assert_snapshot(&loaded, 1, 1);
  remove_dir_if_exists(&directory);
}

#[test]
fn local_snapshot_store_save_then_load_round_trips_payload() {
  let directory = unique_snapshot_dir("round-trip");
  let mut store = open_store(&directory, 3);
  let metadata = SnapshotMetadata::new("pid-1", 1, 10);

  save_snapshot(&mut store, metadata, 42);
  let loaded = load_latest(&store, "pid-1").expect("snapshot should be loaded");

  assert_snapshot(&loaded, 1, 42);
  remove_dir_if_exists(&directory);
}

#[test]
fn local_snapshot_store_save_then_load_preserves_snapshot_metadata() {
  let directory = unique_snapshot_dir("metadata-round-trip");
  let mut store = open_store(&directory, 3);
  let metadata = SnapshotMetadata::new("pid-1", 1, 10).with_metadata("region=ap-northeast-1");

  save_snapshot(&mut store, metadata, 42);
  let loaded = load_latest(&store, "pid-1").expect("snapshot should be loaded");

  assert_eq!(loaded.metadata().metadata(), Some("region=ap-northeast-1"));
  assert_snapshot(&loaded, 1, 42);
  remove_dir_if_exists(&directory);
}

#[test]
fn local_snapshot_store_metadata_sidecar_uses_full_snapshot_file_name() {
  let directory = unique_snapshot_dir("metadata-dotted-pid");
  let mut store = open_store(&directory, 3);
  let persistence_id = "user.1";
  save_snapshot(&mut store, SnapshotMetadata::new(persistence_id, 1, 10).with_metadata("seq=1"), 1);
  save_snapshot(&mut store, SnapshotMetadata::new(persistence_id, 2, 20).with_metadata("seq=2"), 2);

  let criteria = SnapshotSelectionCriteria::new(1, u64::MAX, 1, 0);
  let loaded = poll_ready(store.load_snapshot(persistence_id, criteria))
    .expect("load first snapshot")
    .expect("first snapshot should exist");

  assert_eq!(loaded.metadata().metadata(), Some("seq=1"));
  assert_snapshot(&loaded, 1, 1);
  remove_dir_if_exists(&directory);
}

#[test]
fn local_snapshot_store_overwrite_without_metadata_removes_stale_sidecar() {
  let directory = unique_snapshot_dir("metadata-overwrite-none");
  let mut store = open_store(&directory, 3);
  let metadata_with_sidecar = SnapshotMetadata::new("pid-1", 1, 10).with_metadata("stale");
  save_snapshot(&mut store, metadata_with_sidecar, 1);
  save_snapshot(&mut store, SnapshotMetadata::new("pid-1", 1, 10), 2);

  let loaded = load_latest(&store, "pid-1").expect("snapshot should be loaded");

  assert_eq!(loaded.metadata().metadata(), None);
  assert_snapshot(&loaded, 1, 2);
  remove_dir_if_exists(&directory);
}

#[test]
fn local_snapshot_store_overwrite_with_metadata_replaces_existing_snapshot_and_sidecar() {
  let directory = unique_snapshot_dir("metadata-overwrite-some");
  let mut store = open_store(&directory, 3);
  save_snapshot(&mut store, SnapshotMetadata::new("pid-1", 1, 10).with_metadata("old"), 1);
  save_snapshot(&mut store, SnapshotMetadata::new("pid-1", 1, 10).with_metadata("new"), 2);

  let loaded = load_latest(&store, "pid-1").expect("snapshot should be loaded");

  assert_eq!(loaded.metadata().metadata(), Some("new"));
  assert_snapshot(&loaded, 1, 2);
  remove_dir_if_exists(&directory);
}

#[test]
fn local_snapshot_store_does_not_commit_staged_metadata_when_payload_replace_fails() {
  let directory = unique_snapshot_dir("metadata-stage-payload-fail");
  let mut store = open_store(&directory, 3);
  let metadata = SnapshotMetadata::new("pid-1", 1, 10).with_metadata("new");
  let path = store.snapshot_path(&metadata);
  fs::create_dir(&path).expect("create directory at snapshot path");

  let result = poll_ready(store.save_snapshot(metadata, payload(1)));

  assert!(result.is_err());
  let metadata_path = LocalSnapshotStore::snapshot_metadata_path(&path);
  assert!(!metadata_path.exists());
  assert!(!LocalSnapshotStore::temp_snapshot_path(&metadata_path).exists());
  remove_dir_if_exists(&directory);
}

#[test]
fn local_snapshot_store_rolls_back_payload_when_metadata_commit_fails() {
  let directory = unique_snapshot_dir("metadata-commit-fail");
  let mut store = open_store(&directory, 3);
  let metadata = SnapshotMetadata::new("pid-1", 1, 10).with_metadata("new");
  let path = store.snapshot_path(&metadata);
  let metadata_path = LocalSnapshotStore::snapshot_metadata_path(&path);
  fs::create_dir(&metadata_path).expect("create directory at metadata path");

  let result = poll_ready(store.save_snapshot(metadata, payload(1)));

  assert!(result.is_err());
  assert!(!path.exists());
  assert!(!LocalSnapshotStore::temp_snapshot_path(&metadata_path).exists());
  remove_dir_if_exists(&directory);
}

#[test]
fn local_snapshot_store_load_latest_selects_highest_sequence_number() {
  let directory = unique_snapshot_dir("latest");
  let mut store = open_store(&directory, 3);
  save_snapshot(&mut store, SnapshotMetadata::new("pid-1", 1, 10), 1);
  save_snapshot(&mut store, SnapshotMetadata::new("pid-1", 2, 20), 2);
  save_snapshot(&mut store, SnapshotMetadata::new("pid-2", 3, 30), 30);

  let loaded = load_latest(&store, "pid-1").expect("latest snapshot should exist");

  assert_snapshot(&loaded, 2, 2);
  remove_dir_if_exists(&directory);
}

#[test]
fn local_snapshot_store_load_respects_selection_criteria() {
  let directory = unique_snapshot_dir("criteria");
  let mut store = open_store(&directory, 3);
  save_snapshot(&mut store, SnapshotMetadata::new("pid-1", 1, 10), 1);
  save_snapshot(&mut store, SnapshotMetadata::new("pid-1", 2, 20), 2);

  let criteria = SnapshotSelectionCriteria::new(1, u64::MAX, 0, 0);
  let loaded =
    poll_ready(store.load_snapshot("pid-1", criteria)).expect("load by criteria").expect("snapshot should exist");

  assert_snapshot(&loaded, 1, 1);
  remove_dir_if_exists(&directory);
}

#[test]
fn local_snapshot_store_respects_max_load_attempts_for_corrupt_latest_snapshot() {
  let directory = unique_snapshot_dir("max-attempts");
  let mut store = open_store(&directory, 2);
  save_snapshot(&mut store, SnapshotMetadata::new("pid-1", 1, 10), 1);
  save_snapshot(&mut store, SnapshotMetadata::new("pid-1", 2, 20), 2);
  save_snapshot(&mut store, SnapshotMetadata::new("pid-1", 3, 30), 3);
  fs::write(snapshot_file_for_sequence(&directory, 2), b"corrupt snapshot bytes")
    .expect("corrupt second snapshot file");
  fs::write(snapshot_file_for_sequence(&directory, 3), b"corrupt snapshot bytes")
    .expect("corrupt latest snapshot file");

  let loaded = poll_ready(store.load_snapshot("pid-1", SnapshotSelectionCriteria::latest()));

  assert!(loaded.is_err());
  remove_dir_if_exists(&directory);
}

#[test]
fn local_snapshot_store_falls_back_to_older_snapshot_when_latest_is_corrupt() {
  let directory = unique_snapshot_dir("fallback");
  let mut store = open_store(&directory, 3);
  save_snapshot(&mut store, SnapshotMetadata::new("pid-1", 1, 10), 1);
  save_snapshot(&mut store, SnapshotMetadata::new("pid-1", 2, 20), 2);
  fs::write(snapshot_file_for_sequence(&directory, 2), b"corrupt snapshot bytes")
    .expect("corrupt latest snapshot file");

  let loaded = load_latest(&store, "pid-1").expect("older snapshot should be loaded");

  assert_snapshot(&loaded, 1, 1);
  remove_dir_if_exists(&directory);
}

#[test]
fn local_snapshot_store_delete_snapshots_removes_only_criteria_matches() {
  let directory = unique_snapshot_dir("delete-criteria");
  let mut store = open_store(&directory, 3);
  save_snapshot(&mut store, SnapshotMetadata::new("pid-1", 1, 10), 1);
  save_snapshot(&mut store, SnapshotMetadata::new("pid-1", 2, 20), 2);
  save_snapshot(&mut store, SnapshotMetadata::new("pid-1", 3, 30), 3);

  let criteria = SnapshotSelectionCriteria::new(2, u64::MAX, 0, 0);
  poll_ready(store.delete_snapshots("pid-1", criteria)).expect("delete snapshots by criteria");

  let latest = load_latest(&store, "pid-1").expect("latest snapshot should remain");
  assert_snapshot(&latest, 3, 3);
  let deleted_range = SnapshotSelectionCriteria::new(2, u64::MAX, 0, 0);
  let deleted = poll_ready(store.load_snapshot("pid-1", deleted_range)).expect("load deleted range");
  assert!(deleted.is_none());
  remove_dir_if_exists(&directory);
}

#[test]
fn local_snapshot_store_delete_snapshot_matches_exact_timestamp() {
  let directory = unique_snapshot_dir("delete-exact-timestamp");
  let mut store = open_store(&directory, 3);
  save_snapshot(&mut store, SnapshotMetadata::new("pid-1", 7, 0), 70);
  save_snapshot(&mut store, SnapshotMetadata::new("pid-1", 7, 1234), 7);

  let delete_metadata = SnapshotMetadata::new("pid-1", 7, 0);
  poll_ready(store.delete_snapshot(&delete_metadata)).expect("delete exact snapshot");

  let loaded = load_latest(&store, "pid-1").expect("timestamped snapshot should remain");
  assert_snapshot(&loaded, 7, 7);
  remove_dir_if_exists(&directory);
}

#[test]
fn local_snapshot_store_encodes_persistence_id_before_using_it_in_file_name() {
  let directory = unique_snapshot_dir("encoded-pid");
  let mut store = open_store(&directory, 3);
  let persistence_id = "entity/type 1";

  save_snapshot(&mut store, SnapshotMetadata::new(persistence_id, 1, 10), 99);
  let loaded = load_latest(&store, persistence_id).expect("snapshot with escaped persistence id should load");

  assert_snapshot(&loaded, 1, 99);
  let file_name =
    snapshot_file_for_sequence(&directory, 1).file_name().expect("file name").to_string_lossy().into_owned();
  assert!(!file_name.contains('/'));
  assert!(file_name.contains("entity%2Ftype+1"));
  remove_dir_if_exists(&directory);
}

#[test]
fn local_snapshot_store_percent_encodes_asterisk_in_persistence_id_file_name() {
  let directory = unique_snapshot_dir("encoded-asterisk-pid");
  let mut store = open_store(&directory, 3);
  let persistence_id = "order*42";

  save_snapshot(&mut store, SnapshotMetadata::new(persistence_id, 1, 10), 99);
  let loaded = load_latest(&store, persistence_id).expect("snapshot with escaped persistence id should load");

  assert_snapshot(&loaded, 1, 99);
  let file_name =
    snapshot_file_for_sequence(&directory, 1).file_name().expect("file name").to_string_lossy().into_owned();
  assert!(!file_name.contains('*'));
  assert!(file_name.contains("order%2A42"));
  remove_dir_if_exists(&directory);
}

#[test]
fn local_snapshot_store_percent_encodes_dash_in_persistence_id_file_name() {
  let directory = unique_snapshot_dir("encoded-dash-pid");
  let mut store = open_store(&directory, 3);
  let persistence_id = "order-42";

  save_snapshot(&mut store, SnapshotMetadata::new(persistence_id, 1, 10), 99);
  let loaded = load_latest(&store, persistence_id).expect("snapshot with escaped persistence id should load");

  assert_snapshot(&loaded, 1, 99);
  let file_name =
    snapshot_file_for_sequence(&directory, 1).file_name().expect("file name").to_string_lossy().into_owned();
  assert!(file_name.contains("order%2D42"));
  remove_dir_if_exists(&directory);
}

#[test]
fn local_snapshot_store_loads_pekko_form_urlencoded_snapshot_file_name() {
  let directory = unique_snapshot_dir("pekko-encoded-pid");
  let mut store = open_store(&directory, 3);
  save_snapshot(&mut store, SnapshotMetadata::new("source", 1, 10), 123);
  let source_path = snapshot_file_for_sequence(&directory, 1);
  let pekko_path = directory.join("snapshot-entity%2Ftype+1-1-10");
  fs::rename(source_path, pekko_path).expect("rename snapshot to pekko encoded fixture");

  let loaded = load_latest(&store, "entity/type 1").expect("pekko encoded snapshot should load");

  assert_snapshot(&loaded, 1, 123);
  remove_dir_if_exists(&directory);
}
