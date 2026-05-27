extern crate std;

use std::path::PathBuf;

use fraktor_actor_core_kernel_rs::serialization::{
  builtin, default_serialization_setup, serialization_registry::SerializationRegistry,
};
use fraktor_persistence_core_kernel_rs::serialization::register_persistence_serializers;
use fraktor_utils_core_rs::sync::ArcShared;

use super::LocalSnapshotStoreConfig;

fn serialization_registry() -> ArcShared<SerializationRegistry> {
  let setup = default_serialization_setup();
  let registry = ArcShared::new(SerializationRegistry::from_setup(&setup));
  builtin::register_defaults(&registry, |name, id| panic!("unexpected serializer collision for {name}: {id:?}"))
    .expect("register builtin serializers");
  register_persistence_serializers(&registry).expect("register persistence serializers");
  registry
}

#[test]
fn local_snapshot_store_config_defaults_to_three_load_attempts() {
  let directory = PathBuf::from("target/local-snapshot-store/config-default");

  let config = LocalSnapshotStoreConfig::new(directory.clone(), serialization_registry());

  assert_eq!(config.directory(), directory.as_path());
  assert_eq!(config.max_load_attempts(), 3);
  assert!(config.validate().is_ok());
}

#[test]
fn local_snapshot_store_config_accepts_two_load_attempts() {
  let directory = PathBuf::from("target/local-snapshot-store/config-positive");

  let config = LocalSnapshotStoreConfig::new(directory, serialization_registry()).with_max_load_attempts(2);

  assert_eq!(config.max_load_attempts(), 2);
  assert!(config.validate().is_ok());
}

#[test]
fn local_snapshot_store_config_rejects_one_load_attempt() {
  let directory = PathBuf::from("target/local-snapshot-store/config-one");

  let config = LocalSnapshotStoreConfig::new(directory, serialization_registry()).with_max_load_attempts(1);

  assert!(config.validate().is_err());
}
