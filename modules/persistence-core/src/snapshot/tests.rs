use fraktor_utils_core_rs::sync::ArcShared;

use crate::{snapshot::Snapshot, snapshot_metadata::SnapshotMetadata};

#[test]
fn snapshot_accessors_and_downcast() {
  let metadata = SnapshotMetadata::new("user-1", 9, 30);
  let data = ArcShared::new(42_i32);
  let snapshot = Snapshot::new(metadata.clone(), data);

  assert_eq!(snapshot.metadata(), &metadata);
  assert_eq!(snapshot.downcast_ref::<i32>(), Some(&42));
}
