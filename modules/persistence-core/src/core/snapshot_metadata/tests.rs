use crate::core::snapshot_metadata::SnapshotMetadata;

#[test]
fn snapshot_metadata_accessors() {
  let metadata = SnapshotMetadata::new("user-1", 42, 100);

  assert_eq!(metadata.persistence_id(), "user-1");
  assert_eq!(metadata.sequence_nr(), 42);
  assert_eq!(metadata.timestamp(), 100);
  assert!(metadata.metadata().is_none());
}

#[test]
fn snapshot_metadata_with_metadata() {
  let metadata = SnapshotMetadata::new("user-1", 1, 10).with_metadata("region=ap-northeast-1");

  assert_eq!(metadata.metadata(), Some("region=ap-northeast-1"));
}
