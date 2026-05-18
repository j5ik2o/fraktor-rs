use fraktor_actor_adaptor_std_rs::system::create_noop_actor_system;
use fraktor_actor_core_kernel_rs::actor::{
  ActorContext, Pid,
  error::ActorError,
  messaging::{AnyMessage, AnyMessageView},
};
use fraktor_utils_core_rs::sync::ArcShared;

use crate::{
  persistent::{Eventsourced, PersistentRepr},
  snapshot::{Snapshot, SnapshotError, SnapshotMetadata, SnapshotResponseAction, SnapshotSelectionCriteria},
};

#[derive(Default)]
struct TestEventsourced {
  snapshots:         usize,
  saved:             usize,
  deleted:           usize,
  deleted_by_filter: usize,
  failures:          usize,
}

impl Eventsourced for TestEventsourced {
  fn persistence_id(&self) -> &str {
    "pid-1"
  }

  fn receive_recover(&mut self, _event: &PersistentRepr) {}

  fn receive_snapshot(&mut self, _snapshot: &Snapshot) {
    self.snapshots = self.snapshots.saturating_add(1);
  }

  fn receive_command(&mut self, _ctx: &mut ActorContext<'_>, _message: AnyMessageView<'_>) -> Result<(), ActorError> {
    Ok(())
  }

  fn on_snapshot_saved(&mut self, _metadata: &SnapshotMetadata) {
    self.saved = self.saved.saturating_add(1);
  }

  fn on_snapshot_deleted(&mut self, _metadata: &SnapshotMetadata) {
    self.deleted = self.deleted.saturating_add(1);
  }

  fn on_snapshots_deleted(&mut self, _criteria: &SnapshotSelectionCriteria) {
    self.deleted_by_filter = self.deleted_by_filter.saturating_add(1);
  }

  fn on_snapshot_failure(&mut self, _cause: &SnapshotError) {
    self.failures = self.failures.saturating_add(1);
  }

  fn last_sequence_nr(&self) -> u64 {
    0
  }
}

#[test]
fn snapshot_response_action_apply_routes_to_eventsourced_hooks() {
  let mut actor = TestEventsourced::default();
  let metadata = SnapshotMetadata::new("pid-1", 1, 10);
  let snapshot = Snapshot::new(metadata.clone(), ArcShared::new(1_i32));
  let system = create_noop_actor_system();
  let mut context = ActorContext::new(&system, Pid::new(1, 1));
  let command = AnyMessage::new(1_i32);
  let repr = PersistentRepr::new("pid-1", 1, ArcShared::new(1_i32));

  assert_eq!(actor.persistence_id(), "pid-1");
  actor.receive_recover(&repr);
  actor.receive_command(&mut context, command.as_view()).expect("receive command should succeed");
  SnapshotResponseAction::None.apply(&mut actor);
  SnapshotResponseAction::ReceiveSnapshot(snapshot).apply(&mut actor);
  SnapshotResponseAction::SnapshotSaved(metadata.clone()).apply(&mut actor);
  SnapshotResponseAction::SnapshotDeleted(metadata).apply(&mut actor);
  SnapshotResponseAction::SnapshotsDeleted(SnapshotSelectionCriteria::latest()).apply(&mut actor);
  SnapshotResponseAction::SnapshotFailure(SnapshotError::LoadFailed("boom".into())).apply(&mut actor);

  assert_eq!(actor.snapshots, 1);
  assert_eq!(actor.saved, 1);
  assert_eq!(actor.deleted, 1);
  assert_eq!(actor.deleted_by_filter, 1);
  assert_eq!(actor.failures, 1);
  assert_eq!(actor.last_sequence_nr(), 0);
}
