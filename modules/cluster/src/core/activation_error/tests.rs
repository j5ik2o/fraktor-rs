use crate::core::activation_error::ActivationError;

#[test]
fn snapshot_missing_holds_key() {
  let err = ActivationError::SnapshotMissing { key: "user:1".to_string() };
  assert_eq!(err, ActivationError::SnapshotMissing { key: "user:1".to_string() });
}
