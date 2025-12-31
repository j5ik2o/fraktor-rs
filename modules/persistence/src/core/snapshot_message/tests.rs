use fraktor_actor_rs::core::actor::actor_ref::ActorRef;
use fraktor_utils_rs::core::{runtime_toolbox::NoStdToolbox, sync::ArcShared};

use crate::core::{
  snapshot_message::SnapshotMessage, snapshot_metadata::SnapshotMetadata,
  snapshot_selection_criteria::SnapshotSelectionCriteria,
};

#[test]
fn snapshot_message_variants_hold_data() {
  let sender = ActorRef::null();
  let metadata = SnapshotMetadata::new("pid-1", 1, 10);
  let payload: ArcShared<dyn core::any::Any + Send + Sync> = ArcShared::new(1_i32);

  let save = SnapshotMessage::<NoStdToolbox>::SaveSnapshot {
    metadata: metadata.clone(),
    snapshot: payload,
    sender:   sender.clone(),
  };

  let load = SnapshotMessage::<NoStdToolbox>::LoadSnapshot {
    persistence_id: "pid-1".into(),
    criteria:       SnapshotSelectionCriteria::latest(),
    sender:         sender.clone(),
  };

  let delete_one =
    SnapshotMessage::<NoStdToolbox>::DeleteSnapshot { metadata: metadata.clone(), sender: sender.clone() };

  let delete_many = SnapshotMessage::<NoStdToolbox>::DeleteSnapshots {
    persistence_id: "pid-1".into(),
    criteria: SnapshotSelectionCriteria::none(),
    sender,
  };

  match save {
    | SnapshotMessage::SaveSnapshot { metadata, .. } => assert_eq!(metadata.sequence_nr(), 1),
    | _ => panic!("unexpected variant"),
  }

  match load {
    | SnapshotMessage::LoadSnapshot { persistence_id, .. } => assert_eq!(persistence_id, "pid-1"),
    | _ => panic!("unexpected variant"),
  }

  match delete_one {
    | SnapshotMessage::DeleteSnapshot { metadata, .. } => assert_eq!(metadata.sequence_nr(), 1),
    | _ => panic!("unexpected variant"),
  }

  match delete_many {
    | SnapshotMessage::DeleteSnapshots { persistence_id, .. } => assert_eq!(persistence_id, "pid-1"),
    | _ => panic!("unexpected variant"),
  }
}
