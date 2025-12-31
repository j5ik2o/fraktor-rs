use alloc::string::ToString;

use fraktor_utils_rs::core::sync::ArcShared;

use crate::core::persistent_repr::PersistentRepr;

#[test]
fn persistent_repr_new_and_accessors() {
  let payload: ArcShared<dyn core::any::Any + Send + Sync> = ArcShared::new(7_i32);
  let repr = PersistentRepr::new("pid-1", 3, payload);

  assert_eq!(repr.persistence_id(), "pid-1");
  assert_eq!(repr.sequence_nr(), 3);
  assert_eq!(repr.manifest(), "");
  assert_eq!(repr.writer_uuid(), "");
  assert_eq!(repr.timestamp(), 0);
  assert!(repr.metadata().is_none());
  assert_eq!(repr.downcast_ref::<i32>(), Some(&7));
}

#[test]
fn persistent_repr_with_fields() {
  let payload: ArcShared<dyn core::any::Any + Send + Sync> = ArcShared::new(1_i32);
  let metadata: ArcShared<dyn core::any::Any + Send + Sync> = ArcShared::new("meta".to_string());
  let repr = PersistentRepr::new("pid-1", 1, payload)
    .with_manifest("manifest-1")
    .with_writer_uuid("writer-1")
    .with_timestamp(99)
    .with_metadata(metadata);

  assert_eq!(repr.manifest(), "manifest-1");
  assert_eq!(repr.writer_uuid(), "writer-1");
  assert_eq!(repr.timestamp(), 99);
  assert!(repr.metadata().is_some());
}
