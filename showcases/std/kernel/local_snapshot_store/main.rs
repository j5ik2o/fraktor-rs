use core::{
  any::Any,
  future::Future,
  task::{Context, Poll, Waker},
};
use std::{
  env, fs,
  path::{Path, PathBuf},
  time::{SystemTime, UNIX_EPOCH},
};

use fraktor_actor_core_kernel_rs::serialization::{
  builtin, default_serialization_setup, serialization_registry::SerializationRegistry,
};
use fraktor_persistence_adaptor_std_rs::snapshot::{LocalSnapshotStore, LocalSnapshotStoreConfig};
use fraktor_persistence_core_kernel_rs::{
  serialization::register_persistence_serializers,
  snapshot::{SnapshotMetadata, SnapshotSelectionCriteria, SnapshotStore},
};
use fraktor_utils_core_rs::sync::ArcShared;

fn main() {
  let directory = snapshot_directory();
  let config = LocalSnapshotStoreConfig::new(directory.clone(), serialization_registry());
  let mut store = LocalSnapshotStore::open(config).expect("open local snapshot store");
  let metadata = SnapshotMetadata::new("local-snapshot-store-showcase", 1, 0);
  let payload: ArcShared<dyn Any + Send + Sync> = ArcShared::new(String::from("counter=42"));

  poll_ready(store.save_snapshot(metadata, payload)).expect("save snapshot through kernel trait");
  let loaded = poll_ready(store.load_snapshot("local-snapshot-store-showcase", SnapshotSelectionCriteria::latest()))
    .expect("load snapshot through kernel trait")
    .expect("snapshot should be present");

  println!(
    "loaded snapshot {}:{} = {}",
    loaded.metadata().persistence_id(),
    loaded.metadata().sequence_nr(),
    loaded.downcast_ref::<String>().expect("snapshot payload should be String")
  );

  remove_directory(&directory);
}

fn serialization_registry() -> ArcShared<SerializationRegistry> {
  let setup = default_serialization_setup();
  let registry = ArcShared::new(SerializationRegistry::from_setup(&setup));
  builtin::register_defaults(&registry, |name, id| panic!("serializer collision for {name}: {id:?}"))
    .expect("register builtin serializers");
  register_persistence_serializers(&registry).expect("register persistence serializers");
  registry
}

fn snapshot_directory() -> PathBuf {
  let timestamp =
    SystemTime::now().duration_since(UNIX_EPOCH).expect("system clock should be after unix epoch").as_nanos();
  env::temp_dir().join(format!("fraktor-local-snapshot-store-showcase-{timestamp}"))
}

fn poll_ready<F: Future>(future: F) -> F::Output {
  let waker = Waker::noop();
  let mut context = Context::from_waker(waker);
  let mut future = core::pin::pin!(future);
  match Future::poll(future.as_mut(), &mut context) {
    | Poll::Ready(output) => output,
    | Poll::Pending => panic!("local snapshot store future should complete synchronously"),
  }
}

fn remove_directory(directory: &Path) {
  fs::remove_dir_all(directory).expect("remove local snapshot showcase directory");
}
