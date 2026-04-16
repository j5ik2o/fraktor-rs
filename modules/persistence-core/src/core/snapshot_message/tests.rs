use core::any::Any;

use fraktor_actor_core_rs::core::kernel::actor::actor_ref::ActorRef;
use fraktor_utils_core_rs::core::sync::ArcShared;

use crate::core::{
  snapshot_message::SnapshotMessage, snapshot_metadata::SnapshotMetadata,
  snapshot_selection_criteria::SnapshotSelectionCriteria,
};

#[test]
fn snapshot_message_variants_hold_data() {
  let sender = ActorRef::null();
  let metadata = SnapshotMetadata::new("pid-1", 1, 10);
  let payload: ArcShared<dyn Any + Send + Sync> = ArcShared::new(1_i32);

  let save = SnapshotMessage::SaveSnapshot { metadata: metadata.clone(), snapshot: payload, sender: sender.clone() };

  let load = SnapshotMessage::LoadSnapshot {
    persistence_id: "pid-1".into(),
    criteria:       SnapshotSelectionCriteria::latest(),
    sender:         sender.clone(),
  };

  let delete_one = SnapshotMessage::DeleteSnapshot { metadata: metadata.clone(), sender: sender.clone() };

  let delete_many = SnapshotMessage::DeleteSnapshots {
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
