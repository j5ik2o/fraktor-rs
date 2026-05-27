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
use fraktor_persistence_adaptor_std_rs::snapshot::{LocalSnapshotStore, LocalSnapshotStoreConfig};
use fraktor_persistence_core_kernel_rs::{
  serialization::register_persistence_serializers,
  snapshot::{Snapshot, SnapshotMetadata, SnapshotSelectionCriteria, SnapshotStore},
};
use fraktor_utils_core_rs::sync::ArcShared;

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
  std::env::temp_dir().join(format!("fraktor-local-snapshot-store-flow-{name}-{}-{timestamp}", std::process::id()))
}

fn remove_dir_if_exists(path: &Path) {
  match fs::remove_dir_all(path) {
    | Ok(()) => (),
    | Err(error) if error.kind() == ErrorKind::NotFound => (),
    | Err(error) => panic!("snapshot test directory should be removable: {error}"),
  }
}

fn save_and_load_through_kernel_contract<S: SnapshotStore>(store: &mut S) -> Snapshot {
  let metadata = SnapshotMetadata::new("flow-pid", 1, 10);
  poll_ready(store.save_snapshot(metadata, payload(42))).expect("save through kernel trait");
  poll_ready(store.load_snapshot("flow-pid", SnapshotSelectionCriteria::latest()))
    .expect("load through kernel trait")
    .expect("snapshot should exist")
}

#[test]
fn local_snapshot_store_public_surface_round_trips_through_kernel_snapshot_store_trait() {
  let directory = unique_snapshot_dir("trait-contract");
  let config = LocalSnapshotStoreConfig::new(directory.clone(), serialization_registry());
  let mut store = LocalSnapshotStore::open(config).expect("open local snapshot store");

  let snapshot = save_and_load_through_kernel_contract(&mut store);

  assert_eq!(snapshot.metadata().persistence_id(), "flow-pid");
  assert_eq!(snapshot.metadata().sequence_nr(), 1);
  assert_eq!(snapshot.downcast_ref::<i32>(), Some(&42));
  remove_dir_if_exists(&directory);
}
