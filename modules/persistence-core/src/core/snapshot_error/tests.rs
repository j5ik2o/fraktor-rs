use alloc::string::ToString;

use crate::core::snapshot_error::SnapshotError;

#[test]
fn snapshot_error_display_save_failed() {
  let error = SnapshotError::SaveFailed("io error".into());

  assert_eq!(error.to_string(), "save snapshot failed: io error");
}

#[test]
fn snapshot_error_display_load_failed() {
  let error = SnapshotError::LoadFailed("decode error".into());

  assert_eq!(error.to_string(), "load snapshot failed: decode error");
}

#[test]
fn snapshot_error_display_delete_failed() {
  let error = SnapshotError::DeleteFailed("permission denied".into());

  assert_eq!(error.to_string(), "delete snapshot failed: permission denied");
}
