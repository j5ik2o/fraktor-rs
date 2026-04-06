use fraktor_utils_rs::core::sync::ArcShared;

use crate::core::{
  snapshot::Snapshot, snapshot_error::SnapshotError, snapshot_metadata::SnapshotMetadata,
  snapshot_response::SnapshotResponse, snapshot_selection_criteria::SnapshotSelectionCriteria,
};

#[test]
fn snapshot_response_variants_hold_data() {
  let metadata = SnapshotMetadata::new("pid-1", 1, 10);
  let snapshot = Snapshot::new(metadata.clone(), ArcShared::new(1_i32));

  let save_ok = SnapshotResponse::SaveSnapshotSuccess { metadata: metadata.clone() };
  let save_fail = SnapshotResponse::SaveSnapshotFailure {
    metadata: metadata.clone(),
    error:    SnapshotError::SaveFailed("oops".into()),
  };
  let load_ok = SnapshotResponse::LoadSnapshotResult { snapshot: Some(snapshot.clone()), to_sequence_nr: 5 };
  let load_fail = SnapshotResponse::LoadSnapshotFailed { error: SnapshotError::LoadFailed("bad".into()) };
  let delete_ok = SnapshotResponse::DeleteSnapshotSuccess { metadata: metadata.clone() };
  let delete_many_ok = SnapshotResponse::DeleteSnapshotsSuccess { criteria: SnapshotSelectionCriteria::latest() };
  let delete_fail = SnapshotResponse::DeleteSnapshotFailure {
    metadata: metadata.clone(),
    error:    SnapshotError::DeleteFailed("fail".into()),
  };
  let delete_many_fail = SnapshotResponse::DeleteSnapshotsFailure {
    criteria: SnapshotSelectionCriteria::none(),
    error:    SnapshotError::DeleteFailed("fail".into()),
  };

  match save_ok {
    | SnapshotResponse::SaveSnapshotSuccess { metadata } => assert_eq!(metadata.sequence_nr(), 1),
    | _ => panic!("unexpected variant"),
  }

  match save_fail {
    | SnapshotResponse::SaveSnapshotFailure { metadata, .. } => assert_eq!(metadata.sequence_nr(), 1),
    | _ => panic!("unexpected variant"),
  }

  match load_ok {
    | SnapshotResponse::LoadSnapshotResult { snapshot, to_sequence_nr } => {
      assert_eq!(to_sequence_nr, 5);
      assert!(snapshot.is_some());
    },
    | _ => panic!("unexpected variant"),
  }

  match load_fail {
    | SnapshotResponse::LoadSnapshotFailed { .. } => {},
    | _ => panic!("unexpected variant"),
  }

  match delete_ok {
    | SnapshotResponse::DeleteSnapshotSuccess { metadata } => assert_eq!(metadata.sequence_nr(), 1),
    | _ => panic!("unexpected variant"),
  }

  match delete_many_ok {
    | SnapshotResponse::DeleteSnapshotsSuccess { criteria } => {
      assert_eq!(criteria, SnapshotSelectionCriteria::latest());
    },
    | _ => panic!("unexpected variant"),
  }

  match delete_fail {
    | SnapshotResponse::DeleteSnapshotFailure { metadata, .. } => assert_eq!(metadata.sequence_nr(), 1),
    | _ => panic!("unexpected variant"),
  }

  match delete_many_fail {
    | SnapshotResponse::DeleteSnapshotsFailure { criteria, .. } => {
      assert_eq!(criteria, SnapshotSelectionCriteria::none())
    },
    | _ => panic!("unexpected variant"),
  }
}
