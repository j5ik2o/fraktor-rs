use fraktor_utils_core_rs::sync::ArcShared;

use crate::snapshot::{Snapshot, SnapshotMetadata, SnapshotOffer};

#[test]
fn snapshot_offer_exposes_snapshot_metadata_and_payload() {
  let metadata = SnapshotMetadata::new("pid-1", 4, 10);
  let snapshot = Snapshot::new(metadata.clone(), ArcShared::new(99_i32));
  let offer = SnapshotOffer::new(snapshot);

  assert_eq!(offer.metadata(), &metadata);
  assert_eq!(offer.downcast_ref::<i32>(), Some(&99));
}
