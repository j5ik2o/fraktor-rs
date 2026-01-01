use fraktor_actor_rs::core::{
  actor::{
    ActorContextGeneric, Pid,
    actor_ref::{ActorRef, ActorRefGeneric, ActorRefSender, SendOutcome},
  },
  error::{ActorError, SendError},
  messaging::{AnyMessageGeneric, AnyMessageViewGeneric},
};
use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdToolbox, RuntimeToolbox, SyncMutexFamily, ToolboxMutex},
  sync::ArcShared,
};

use crate::core::{
  eventsourced::Eventsourced, journal_error::JournalError, journal_message::JournalMessage,
  journal_response::JournalResponse, persistent_actor_base::PersistentActorBase,
  persistent_actor_state::PersistentActorState, persistent_repr::PersistentRepr, recovery::Recovery,
  snapshot::Snapshot, snapshot_error::SnapshotError, snapshot_metadata::SnapshotMetadata,
  snapshot_response::SnapshotResponse,
};

type TB = NoStdToolbox;
type MessageStore = ArcShared<ToolboxMutex<Vec<AnyMessageGeneric<TB>>, TB>>;

struct TestSender {
  messages: MessageStore,
}

impl ActorRefSender<TB> for TestSender {
  fn send(&mut self, message: AnyMessageGeneric<TB>) -> Result<SendOutcome, SendError<TB>> {
    self.messages.lock().push(message);
    Ok(SendOutcome::Delivered)
  }
}

fn create_sender() -> (ActorRefGeneric<TB>, MessageStore) {
  let messages = ArcShared::new(<<NoStdToolbox as RuntimeToolbox>::MutexFamily as SyncMutexFamily>::create(Vec::new()));
  let sender = ActorRefGeneric::new(Pid::new(1, 1), TestSender { messages: messages.clone() });
  (sender, messages)
}

struct DummyActor {
  calls:              usize,
  persist_failures:   usize,
  snapshot_failures:  usize,
  recovery_completed: usize,
  recovery_failures:  usize,
  persistence_id:     String,
  journal_ref:        ActorRefGeneric<TB>,
  snapshot_ref:       ActorRefGeneric<TB>,
  recovery:           Recovery,
}

impl DummyActor {
  fn new(
    persistence_id: String,
    journal_ref: ActorRefGeneric<TB>,
    snapshot_ref: ActorRefGeneric<TB>,
    recovery: Recovery,
  ) -> Self {
    Self {
      calls: 0,
      persist_failures: 0,
      snapshot_failures: 0,
      recovery_completed: 0,
      recovery_failures: 0,
      persistence_id,
      journal_ref,
      snapshot_ref,
      recovery,
    }
  }
}

impl Eventsourced<TB> for DummyActor {
  fn persistence_id(&self) -> &str {
    &self.persistence_id
  }

  fn journal_actor_ref(&self) -> &ActorRefGeneric<TB> {
    &self.journal_ref
  }

  fn snapshot_actor_ref(&self) -> &ActorRefGeneric<TB> {
    &self.snapshot_ref
  }

  fn receive_recover(&mut self, _event: &PersistentRepr) {}

  fn receive_snapshot(&mut self, _snapshot: &Snapshot) {}

  fn receive_command(
    &mut self,
    _ctx: &mut ActorContextGeneric<'_, TB>,
    _message: AnyMessageViewGeneric<'_, TB>,
  ) -> Result<(), ActorError> {
    Ok(())
  }

  fn last_sequence_nr(&self) -> u64 {
    0
  }

  fn recovery(&self) -> Recovery {
    self.recovery.clone()
  }

  fn on_recovery_failure(&mut self, _cause: &crate::core::persistence_error::PersistenceError) {
    self.recovery_failures += 1;
  }

  fn on_persist_failure(&mut self, _cause: &JournalError, _repr: &PersistentRepr) {
    self.persist_failures += 1;
  }

  fn on_snapshot_failure(&mut self, _cause: &SnapshotError) {
    self.snapshot_failures += 1;
  }

  fn on_recovery_completed(&mut self) {
    self.recovery_completed += 1;
  }
}

#[test]
fn persistent_actor_base_flush_batch_sends_write_messages() {
  let (journal_ref, store) = create_sender();
  let snapshot_ref = ActorRef::null();
  let mut base = PersistentActorBase::<DummyActor, TB>::new("pid-1".into(), journal_ref, snapshot_ref);
  base.state = PersistentActorState::ProcessingCommands;
  base.add_to_event_batch(1_i32, true, Box::new(|_, _| {}));
  base.flush_batch(ActorRef::null());

  let messages = store.lock();
  assert_eq!(messages.len(), 1);
  let message = messages[0].payload().downcast_ref::<JournalMessage<TB>>().expect("unexpected payload");
  match message {
    | JournalMessage::WriteMessages { to_sequence_nr, messages, .. } => {
      assert_eq!(*to_sequence_nr, 1);
      assert_eq!(messages.len(), 1);
    },
    | _ => panic!("unexpected message"),
  }
}

#[test]
fn persistent_actor_base_handle_journal_response_invokes_handler() {
  let (journal_ref, _store) = create_sender();
  let snapshot_ref = ActorRef::null();
  let mut base = PersistentActorBase::<DummyActor, TB>::new("pid-1".into(), journal_ref, snapshot_ref);
  base.state = PersistentActorState::ProcessingCommands;

  let mut actor = DummyActor::new("pid-1".into(), ActorRef::null(), ActorRef::null(), Recovery::default());
  base.add_to_event_batch(
    1_i32,
    true,
    Box::new(|actor: &mut DummyActor, _| {
      actor.calls += 1;
    }),
  );
  base.flush_batch(ActorRef::null());
  base.state = PersistentActorState::PersistingEvents;

  let payload: ArcShared<dyn core::any::Any + Send + Sync> = ArcShared::new(1_i32);
  let repr = PersistentRepr::new("pid-1", 1, payload);
  let response = JournalResponse::WriteMessageSuccess { repr, instance_id: 1 };
  let action = base.handle_journal_response(&response);
  action.apply::<TB>(&mut actor);

  assert_eq!(actor.calls, 1);
  assert_eq!(base.state(), PersistentActorState::ProcessingCommands);
}

#[test]
fn persistent_actor_base_start_recovery_none_requests_highest_sequence() {
  let (journal_ref, store) = create_sender();
  let snapshot_ref = ActorRef::null();
  let mut base = PersistentActorBase::<DummyActor, TB>::new("pid-1".into(), journal_ref, snapshot_ref);
  let actor = DummyActor::new("pid-1".into(), ActorRef::null(), ActorRef::null(), Recovery::none());

  base.start_recovery(actor.recovery(), ActorRef::null());

  assert_eq!(base.state(), PersistentActorState::Recovering);
  let messages = store.lock();
  assert_eq!(messages.len(), 1);
  let message = messages[0].payload().downcast_ref::<JournalMessage<TB>>().expect("unexpected payload");
  match message {
    | JournalMessage::GetHighestSequenceNr { persistence_id, .. } => {
      assert_eq!(persistence_id, "pid-1");
    },
    | _ => panic!("unexpected message"),
  }
}

#[test]
fn persistent_actor_base_handle_snapshot_response_transitions_and_replays() {
  let (journal_ref, store) = create_sender();
  let snapshot_ref = ActorRef::null();
  let mut base = PersistentActorBase::<DummyActor, TB>::new("pid-1".into(), journal_ref, snapshot_ref);
  base.state = PersistentActorState::RecoveryStarted;
  base.recovery = Recovery::default();
  let mut actor = DummyActor::new("pid-1".into(), ActorRef::null(), ActorRef::null(), Recovery::default());

  let response = SnapshotResponse::LoadSnapshotResult { snapshot: None, to_sequence_nr: 0 };
  let action = base.handle_snapshot_response(&response, ActorRef::null());
  action.apply::<TB>(&mut actor);

  assert_eq!(base.state(), PersistentActorState::Recovering);
  let messages = store.lock();
  assert_eq!(messages.len(), 1);
  let message = messages[0].payload().downcast_ref::<JournalMessage<TB>>().expect("unexpected payload");
  match message {
    | JournalMessage::ReplayMessages { from_sequence_nr, to_sequence_nr, .. } => {
      assert_eq!(*from_sequence_nr, 0);
      assert_eq!(*to_sequence_nr, base.recovery.to_sequence_nr());
    },
    | _ => panic!("unexpected message"),
  }
}

#[test]
fn persistent_actor_base_recovery_keeps_snapshot_sequence_when_no_replay() {
  let (journal_ref, _store) = create_sender();
  let snapshot_ref = ActorRef::null();
  let mut base = PersistentActorBase::<DummyActor, TB>::new("pid-1".into(), journal_ref, snapshot_ref);
  base.state = PersistentActorState::RecoveryStarted;
  base.recovery = Recovery::default();
  let mut actor = DummyActor::new("pid-1".into(), ActorRef::null(), ActorRef::null(), Recovery::default());

  let metadata = SnapshotMetadata::new("pid-1", 10, 0);
  let snapshot = Snapshot::new(metadata, ArcShared::new(1_i32));
  let response = SnapshotResponse::LoadSnapshotResult { snapshot: Some(snapshot), to_sequence_nr: 10 };
  let action = base.handle_snapshot_response(&response, ActorRef::null());
  action.apply::<TB>(&mut actor);

  assert_eq!(base.current_sequence_nr(), 10);
  assert_eq!(base.last_sequence_nr(), 10);

  let response = JournalResponse::RecoverySuccess { highest_sequence_nr: 0 };
  let action = base.handle_journal_response(&response);
  action.apply::<TB>(&mut actor);

  assert_eq!(base.current_sequence_nr(), 10);
  assert_eq!(base.last_sequence_nr(), 10);
}

#[test]
fn persistent_actor_base_handle_journal_response_calls_failure_hook() {
  let (journal_ref, _store) = create_sender();
  let snapshot_ref = ActorRef::null();
  let mut base = PersistentActorBase::<DummyActor, TB>::new("pid-1".into(), journal_ref, snapshot_ref);
  base.state = PersistentActorState::PersistingEvents;
  let mut actor = DummyActor::new("pid-1".into(), ActorRef::null(), ActorRef::null(), Recovery::default());

  let payload: ArcShared<dyn core::any::Any + Send + Sync> = ArcShared::new(1_i32);
  let repr = PersistentRepr::new("pid-1", 1, payload);
  let response =
    JournalResponse::WriteMessageFailure { repr, cause: JournalError::WriteFailed("boom".into()), instance_id: 1 };

  let action = base.handle_journal_response(&response);
  action.apply::<TB>(&mut actor);

  assert_eq!(actor.persist_failures, 1);
}

#[test]
fn persistent_actor_base_handle_snapshot_failure_calls_hook() {
  let (journal_ref, _store) = create_sender();
  let snapshot_ref = ActorRef::null();
  let mut base = PersistentActorBase::<DummyActor, TB>::new("pid-1".into(), journal_ref, snapshot_ref);
  base.state = PersistentActorState::RecoveryStarted;
  let mut actor = DummyActor::new("pid-1".into(), ActorRef::null(), ActorRef::null(), Recovery::default());

  let response = SnapshotResponse::LoadSnapshotFailed { error: SnapshotError::LoadFailed("boom".into()) };
  let action = base.handle_snapshot_response(&response, ActorRef::null());
  action.apply::<TB>(&mut actor);

  assert_eq!(actor.snapshot_failures, 1);
}

#[test]
fn persistent_actor_base_snapshot_failure_continues_recovery() {
  let (journal_ref, store) = create_sender();
  let snapshot_ref = ActorRef::null();
  let mut base = PersistentActorBase::<DummyActor, TB>::new("pid-1".into(), journal_ref, snapshot_ref);
  base.state = PersistentActorState::RecoveryStarted;
  base.recovery = Recovery::default();
  let mut actor = DummyActor::new("pid-1".into(), ActorRef::null(), ActorRef::null(), Recovery::default());

  let response = SnapshotResponse::LoadSnapshotFailed { error: SnapshotError::LoadFailed("boom".into()) };
  let action = base.handle_snapshot_response(&response, ActorRef::null());
  action.apply::<TB>(&mut actor);

  assert_eq!(actor.snapshot_failures, 1);
  assert_eq!(base.state(), PersistentActorState::Recovering);
  let messages = store.lock();
  assert_eq!(messages.len(), 1);
  let message = messages[0].payload().downcast_ref::<JournalMessage<TB>>().expect("unexpected payload");
  match message {
    | JournalMessage::ReplayMessages { from_sequence_nr, .. } => {
      assert_eq!(*from_sequence_nr, 0);
    },
    | _ => panic!("unexpected message"),
  }
}

#[test]
fn persistent_actor_base_handle_highest_sequence_completes_recovery() {
  let (journal_ref, _store) = create_sender();
  let snapshot_ref = ActorRef::null();
  let mut base = PersistentActorBase::<DummyActor, TB>::new("pid-1".into(), journal_ref, snapshot_ref);
  base.state = PersistentActorState::Recovering;
  let mut actor = DummyActor::new("pid-1".into(), ActorRef::null(), ActorRef::null(), Recovery::default());

  let response = JournalResponse::HighestSequenceNr { persistence_id: "pid-1".into(), sequence_nr: 42 };
  let action = base.handle_journal_response(&response);
  action.apply::<TB>(&mut actor);

  assert_eq!(actor.recovery_completed, 1);
  assert_eq!(base.state(), PersistentActorState::ProcessingCommands);
  assert_eq!(base.last_sequence_nr(), 42);
}

#[test]
fn persistent_actor_base_handle_highest_sequence_failure_calls_hook() {
  let (journal_ref, _store) = create_sender();
  let snapshot_ref = ActorRef::null();
  let mut base = PersistentActorBase::<DummyActor, TB>::new("pid-1".into(), journal_ref, snapshot_ref);
  base.state = PersistentActorState::Recovering;
  let mut actor = DummyActor::new("pid-1".into(), ActorRef::null(), ActorRef::null(), Recovery::default());

  let response = JournalResponse::HighestSequenceNrFailure {
    persistence_id: "pid-1".into(),
    cause:          JournalError::ReadFailed("boom".into()),
  };
  let action = base.handle_journal_response(&response);
  action.apply::<TB>(&mut actor);

  assert_eq!(actor.recovery_failures, 1);
}

#[test]
fn persistent_actor_base_persist_failure_drops_pending_invocation() {
  let (journal_ref, _store) = create_sender();
  let snapshot_ref = ActorRef::null();
  let mut base = PersistentActorBase::<DummyActor, TB>::new("pid-1".into(), journal_ref, snapshot_ref);
  base.state = PersistentActorState::ProcessingCommands;

  let mut actor = DummyActor::new("pid-1".into(), ActorRef::null(), ActorRef::null(), Recovery::default());
  base.add_to_event_batch(
    1_i32,
    true,
    Box::new(|actor: &mut DummyActor, _| {
      actor.calls += 10;
    }),
  );
  base.add_to_event_batch(
    2_i32,
    true,
    Box::new(|actor: &mut DummyActor, _| {
      actor.calls += 1;
    }),
  );
  base.flush_batch(ActorRef::null());
  base.state = PersistentActorState::PersistingEvents;

  let payload1: ArcShared<dyn core::any::Any + Send + Sync> = ArcShared::new(1_i32);
  let repr1 = PersistentRepr::new("pid-1", 1, payload1);
  let failure = JournalResponse::WriteMessageFailure {
    repr:        repr1,
    cause:       JournalError::WriteFailed("boom".into()),
    instance_id: 1,
  };
  let action = base.handle_journal_response(&failure);
  action.apply::<TB>(&mut actor);
  assert_eq!(actor.calls, 0);

  let payload2: ArcShared<dyn core::any::Any + Send + Sync> = ArcShared::new(2_i32);
  let repr2 = PersistentRepr::new("pid-1", 2, payload2);
  let success = JournalResponse::WriteMessageSuccess { repr: repr2, instance_id: 1 };
  let action = base.handle_journal_response(&success);
  action.apply::<TB>(&mut actor);

  assert_eq!(actor.calls, 1);
}
