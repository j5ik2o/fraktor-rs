use core::time::Duration;

use fraktor_actor_adaptor_std_rs::system::create_noop_actor_system;
use fraktor_actor_core_kernel_rs::actor::{
  ActorContext, Pid,
  error::ActorError,
  messaging::{AnyMessage, AnyMessageView},
};
use fraktor_utils_core_rs::sync::ArcShared;

use crate::{
  error::PersistenceError,
  journal::JournalError,
  persistent::{Eventsourced, PersistentRepr, Recovery, RecoveryTimedOut},
  snapshot::{Snapshot, SnapshotError, SnapshotMetadata, SnapshotOffer, SnapshotSelectionCriteria},
};

struct DummyEventsourced {
  persistence_id: String,
  last:           u64,
  snapshots:      usize,
}

impl Eventsourced for DummyEventsourced {
  fn persistence_id(&self) -> &str {
    &self.persistence_id
  }

  fn receive_recover(&mut self, _event: &PersistentRepr) {}

  fn receive_snapshot(&mut self, _snapshot: &Snapshot) {
    self.snapshots = self.snapshots.saturating_add(1);
  }

  fn receive_command(&mut self, _ctx: &mut ActorContext<'_>, _message: AnyMessageView<'_>) -> Result<(), ActorError> {
    Ok(())
  }

  fn last_sequence_nr(&self) -> u64 {
    self.last
  }
}

#[test]
fn eventsourced_default_recovery_is_latest() {
  let dummy = DummyEventsourced { persistence_id: "pid-1".into(), last: 0, snapshots: 0 };

  let recovery = dummy.recovery();
  assert_eq!(recovery, Recovery::default());
  assert_eq!(dummy.recovery_event_timeout(), Duration::from_secs(30));
}

#[test]
fn eventsourced_default_hooks_do_not_panic() {
  let mut dummy = DummyEventsourced { persistence_id: "pid-1".into(), last: 0, snapshots: 0 };
  let system = create_noop_actor_system();
  let pid = Pid::new(1, 1);
  let mut ctx = ActorContext::new(&system, pid);
  let message = AnyMessage::new(1_i32);
  let repr = PersistentRepr::new("pid-1", 1, ArcShared::new(1_i32));
  let journal_error = JournalError::WriteFailed("boom".into());
  let persistence_error = PersistenceError::Recovery("boom".into());
  let snapshot_error = SnapshotError::SaveFailed("boom".into());
  let metadata = SnapshotMetadata::new("pid-1", 1, 10);
  let criteria = SnapshotSelectionCriteria::latest();

  dummy.receive_command(&mut ctx, message.as_view()).expect("receive command should succeed");
  dummy.on_recovery_completed();
  dummy.on_recovery_timed_out(&RecoveryTimedOut::new("pid-1"));
  dummy.on_persist_failure(&journal_error, &repr);
  assert!(matches!(dummy.persist_failure_error(&journal_error, &repr), ActorError::Fatal(_)));
  dummy.on_persist_rejected(&journal_error, &repr);
  dummy.on_recovery_failure(&persistence_error);
  dummy.on_snapshot_failure(&snapshot_error);
  dummy.on_snapshot_saved(&metadata);
  dummy.on_snapshot_deleted(&metadata);
  dummy.on_snapshots_deleted(&criteria);
  dummy.on_events_deleted(10);
  dummy.on_events_delete_failure(&journal_error, 10);
}

#[test]
fn eventsourced_default_snapshot_offer_hook_forwards_to_snapshot_callback() {
  let mut dummy = DummyEventsourced { persistence_id: "pid-1".into(), last: 0, snapshots: 0 };
  let metadata = SnapshotMetadata::new("pid-1", 1, 10);
  let snapshot = Snapshot::new(metadata, ArcShared::new(1_i32));
  let offer = SnapshotOffer::new(snapshot);

  dummy.receive_snapshot_offer(&offer);

  assert_eq!(dummy.snapshots, 1);
}
