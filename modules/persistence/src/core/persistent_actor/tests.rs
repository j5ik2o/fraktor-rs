use fraktor_actor_rs::core::kernel::{
  actor::{
    Actor, ActorCell, ActorContext, Pid,
    actor_ref::{ActorRef, ActorRefSender, SendOutcome},
    error::{ActorError, SendError},
    messaging::{AnyMessage, AnyMessageView},
    props::Props,
  },
  system::{
    ActorSystem,
    state::{SystemStateShared, system_state::SystemState},
  },
};
use fraktor_utils_rs::core::sync::{ArcShared, RuntimeMutex};

use crate::core::{
  eventsourced::Eventsourced, journal_message::JournalMessage, journal_response::JournalResponse,
  persistence_context::PersistenceContext, persistent_actor::PersistentActor, persistent_repr::PersistentRepr,
  snapshot::Snapshot, snapshot_message::SnapshotMessage, snapshot_metadata::SnapshotMetadata,
  snapshot_response::SnapshotResponse, snapshot_selection_criteria::SnapshotSelectionCriteria,
};

type MessageStore = ArcShared<RuntimeMutex<Vec<AnyMessage>>>;

struct TestSender {
  messages: MessageStore,
}

impl ActorRefSender for TestSender {
  fn send(&mut self, message: AnyMessage) -> Result<SendOutcome, SendError> {
    self.messages.lock().push(message);
    Ok(SendOutcome::Delivered)
  }
}

fn create_sender_with_pid(pid: Pid) -> (ActorRef, MessageStore) {
  let messages = ArcShared::new(RuntimeMutex::new(Vec::new()));
  let sender = ActorRef::new(pid, TestSender { messages: messages.clone() });
  (sender, messages)
}

fn create_sender() -> (ActorRef, MessageStore) {
  create_sender_with_pid(Pid::new(1, 1))
}

struct NoopActor;

impl Actor for NoopActor {
  fn receive(&mut self, _ctx: &mut ActorContext<'_>, _message: AnyMessageView<'_>) -> Result<(), ActorError> {
    Ok(())
  }
}

fn build_context() -> ActorContext<'static> {
  let state = SystemStateShared::new(SystemState::new());
  let system = ActorSystem::from_state(state.clone());
  let pid = system.allocate_pid();
  let props = Props::from_fn(|| NoopActor);
  let cell = ActorCell::create(state.clone(), pid, None, "test".into(), &props).expect("actor cell should be created");
  state.register_cell(cell);
  ActorContext::new(&system, pid)
}

fn start_recovery_without_snapshot(actor: &mut DummyPersistentActor) {
  let (journal_ref, _journal_store) = create_sender();
  let (snapshot_ref, _snapshot_store) = create_sender();
  actor.context.bind_actor_refs(journal_ref, snapshot_ref).expect("bind actor refs");
  actor.context.start_recovery(crate::core::Recovery::default(), ActorRef::null()).expect("start recovery");
}

fn move_to_recovering(actor: &mut DummyPersistentActor) {
  start_recovery_without_snapshot(actor);
  let _ = actor.context.handle_snapshot_response(
    &SnapshotResponse::LoadSnapshotResult { snapshot: None, to_sequence_nr: u64::MAX },
    ActorRef::null(),
  );
}

fn move_to_processing_commands(actor: &mut DummyPersistentActor) {
  actor.context.start_recovery(crate::core::Recovery::default(), ActorRef::null()).expect("start recovery");
  let _ = actor.context.handle_snapshot_response(
    &SnapshotResponse::LoadSnapshotResult { snapshot: None, to_sequence_nr: u64::MAX },
    ActorRef::null(),
  );
  let _ = actor.context.handle_journal_response(&JournalResponse::RecoverySuccess { highest_sequence_nr: 0 });
}

struct DummyPersistentActor {
  context: PersistenceContext<DummyPersistentActor>,
  handled: Vec<i32>,
}

struct NonCloneEvent {
  value: i32,
}

impl DummyPersistentActor {
  fn new() -> Self {
    Self { context: PersistenceContext::new("pid-1".into()), handled: Vec::new() }
  }

  fn new_with_refs(journal: ActorRef, snapshot: ActorRef) -> Self {
    let mut actor = Self::new();
    let _ = actor.context.bind_actor_refs(journal, snapshot);
    actor
  }
}

impl Eventsourced for DummyPersistentActor {
  fn persistence_id(&self) -> &str {
    self.context.persistence_id()
  }

  fn receive_recover(&mut self, _event: &PersistentRepr) {}

  fn receive_snapshot(&mut self, _snapshot: &Snapshot) {}

  fn receive_command(&mut self, _ctx: &mut ActorContext<'_>, _message: AnyMessageView<'_>) -> Result<(), ActorError> {
    Ok(())
  }

  fn last_sequence_nr(&self) -> u64 {
    self.context.last_sequence_nr()
  }
}

impl PersistentActor for DummyPersistentActor {
  fn persistence_context(&mut self) -> &mut PersistenceContext<Self> {
    &mut self.context
  }
}

#[test]
fn persistent_actor_persist_increments_sequence() {
  let mut ctx = build_context();
  let mut actor = DummyPersistentActor::new();

  actor.persist(&mut ctx, 1_i32, |_actor, _| {});

  assert_eq!(actor.context.current_sequence_nr(), 1);
}

#[test]
fn persistent_actor_persist_all_increments_sequence() {
  let mut ctx = build_context();
  let mut actor = DummyPersistentActor::new();

  actor.persist_all(&mut ctx, vec![1_i32, 2_i32, 3_i32], |_actor, _| {});

  assert_eq!(actor.context.current_sequence_nr(), 3);
}

#[test]
fn persistent_actor_persist_async_increments_sequence() {
  let mut ctx = build_context();
  let mut actor = DummyPersistentActor::new();

  actor.persist_async(&mut ctx, 1_i32, |_actor, _| {});

  assert_eq!(actor.context.current_sequence_nr(), 1);
}

#[test]
fn persistent_actor_persist_all_async_increments_sequence() {
  let mut ctx = build_context();
  let mut actor = DummyPersistentActor::new();

  actor.persist_all_async(&mut ctx, vec![1_i32, 2_i32, 3_i32], |_actor, _| {});

  assert_eq!(actor.context.current_sequence_nr(), 3);
}

#[test]
fn persistent_actor_persist_all_accepts_non_clone_events() {
  let mut ctx = build_context();
  let mut actor = DummyPersistentActor::new();
  let events = vec![NonCloneEvent { value: 1 }, NonCloneEvent { value: 2 }];

  actor.persist_all(&mut ctx, events, |_actor, event| {
    assert!(event.value > 0);
  });

  assert_eq!(actor.context.current_sequence_nr(), 2);
}

#[test]
fn persistent_actor_persist_all_async_accepts_non_clone_events() {
  let mut ctx = build_context();
  let mut actor = DummyPersistentActor::new();
  let events = vec![NonCloneEvent { value: 1 }, NonCloneEvent { value: 2 }];

  actor.persist_all_async(&mut ctx, events, |_actor, event| {
    assert!(event.value > 0);
  });

  assert_eq!(actor.context.current_sequence_nr(), 2);
}

#[test]
fn persistent_actor_clears_sender_in_journal_representations() {
  let (journal_ref, journal_store) = create_sender();
  let (snapshot_ref, _snapshot_store) = create_sender();
  let (sender_ref, _sender_store) = create_sender_with_pid(Pid::new(9, 9));
  let mut ctx = build_context();
  let mut actor = DummyPersistentActor::new_with_refs(journal_ref, snapshot_ref);
  move_to_processing_commands(&mut actor);

  ctx.set_sender(Some(sender_ref.clone()));
  actor.persist(&mut ctx, 1_i32, |_actor, _| {});
  actor.persist_unfenced(&mut ctx, 2_i32, |_actor, _| {});
  actor.persist_all(&mut ctx, vec![3_i32, 4_i32], |_actor, _| {});
  actor.persist_all_async(&mut ctx, vec![5_i32, 6_i32], |_actor, _| {});
  actor.flush_batch(&mut ctx).expect("flush");

  let messages = journal_store.lock();
  let message = messages
    .iter()
    .filter_map(|message| message.payload().downcast_ref::<JournalMessage>())
    .find(|message| matches!(message, JournalMessage::WriteMessages { .. }))
    .expect("write messages not found");
  match message {
    | JournalMessage::WriteMessages { messages, .. } => {
      assert_eq!(messages.len(), 6);
      for repr in messages {
        assert_eq!(repr.sender(), None);
      }
    },
    | _ => panic!("unexpected message"),
  }
}

#[test]
fn persistent_actor_save_snapshot_sends_message() {
  let (journal_ref, _journal_store) = create_sender();
  let (snapshot_ref, snapshot_store) = create_sender();
  let mut ctx = build_context();
  let mut actor = DummyPersistentActor::new_with_refs(journal_ref, snapshot_ref);

  let snapshot: ArcShared<dyn core::any::Any + Send + Sync> = ArcShared::new(123_u32);
  actor.save_snapshot(&mut ctx, snapshot).expect("save_snapshot failed");

  let messages = snapshot_store.lock();
  assert_eq!(messages.len(), 1);
  let message = messages[0].payload().downcast_ref::<SnapshotMessage>().expect("unexpected payload");
  match message {
    | SnapshotMessage::SaveSnapshot { metadata, .. } => {
      assert_eq!(metadata.persistence_id(), "pid-1");
      assert_eq!(metadata.sequence_nr(), actor.context.current_sequence_nr());
    },
    | _ => panic!("unexpected message"),
  }
}

#[test]
fn persistent_actor_delete_messages_sends_message() {
  let (journal_ref, journal_store) = create_sender();
  let (snapshot_ref, _snapshot_store) = create_sender();
  let mut ctx = build_context();
  let mut actor = DummyPersistentActor::new_with_refs(journal_ref, snapshot_ref);

  actor.delete_messages(&mut ctx, 10).expect("delete_messages failed");

  let messages = journal_store.lock();
  assert_eq!(messages.len(), 1);
  let message = messages[0].payload().downcast_ref::<JournalMessage>().expect("unexpected payload");
  match message {
    | JournalMessage::DeleteMessagesTo { persistence_id, to_sequence_nr, .. } => {
      assert_eq!(persistence_id, "pid-1");
      assert_eq!(*to_sequence_nr, 10);
    },
    | _ => panic!("unexpected message"),
  }
}

#[test]
fn persistent_actor_delete_snapshots_sends_message() {
  let (journal_ref, _journal_store) = create_sender();
  let (snapshot_ref, snapshot_store) = create_sender();
  let mut ctx = build_context();
  let mut actor = DummyPersistentActor::new_with_refs(journal_ref, snapshot_ref);

  let criteria = SnapshotSelectionCriteria::latest();
  actor.delete_snapshots(&mut ctx, criteria.clone()).expect("delete_snapshots failed");

  let messages = snapshot_store.lock();
  assert_eq!(messages.len(), 1);
  let message = messages[0].payload().downcast_ref::<SnapshotMessage>().expect("unexpected payload");
  match message {
    | SnapshotMessage::DeleteSnapshots { persistence_id, criteria: sent, .. } => {
      assert_eq!(persistence_id, "pid-1");
      assert_eq!(sent, &criteria);
    },
    | _ => panic!("unexpected message"),
  }
}

#[test]
fn persistent_actor_delete_snapshot_sends_message() {
  let (journal_ref, _journal_store) = create_sender();
  let (snapshot_ref, snapshot_store) = create_sender();
  let mut ctx = build_context();
  let mut actor = DummyPersistentActor::new_with_refs(journal_ref, snapshot_ref);

  actor.delete_snapshot(&mut ctx, 7).expect("delete_snapshot failed");

  let messages = snapshot_store.lock();
  assert_eq!(messages.len(), 1);
  let message = messages[0].payload().downcast_ref::<SnapshotMessage>().expect("unexpected payload");
  match message {
    | SnapshotMessage::DeleteSnapshot { metadata: sent, .. } => {
      assert_eq!(sent, &SnapshotMetadata::new("pid-1", 7, 0));
    },
    | _ => panic!("unexpected message"),
  }
}

#[test]
fn persistent_actor_defer_runs_after_write_messages_successful() {
  let (journal_ref, journal_store) = create_sender();
  let (snapshot_ref, _snapshot_store) = create_sender();
  let mut ctx = build_context();
  let mut actor = DummyPersistentActor::new_with_refs(journal_ref, snapshot_ref);
  actor.context.start_recovery(crate::core::Recovery::default(), ActorRef::null()).expect("start recovery");
  let _ = actor.context.handle_snapshot_response(
    &SnapshotResponse::LoadSnapshotResult { snapshot: None, to_sequence_nr: u64::MAX },
    ActorRef::null(),
  );
  let _ = actor.context.handle_journal_response(&JournalResponse::RecoverySuccess { highest_sequence_nr: 0 });

  actor.persist(&mut ctx, 1_i32, |actor, event| {
    actor.handled.push(*event);
  });
  actor.defer(&mut ctx, 2_i32, |actor, event| {
    actor.handled.push(*event);
  });
  actor.flush_batch(&mut ctx).expect("flush");

  let repr = {
    let messages = journal_store.lock();
    let maybe_repr =
      messages.iter().filter_map(|message| message.payload().downcast_ref::<JournalMessage>()).find_map(|message| {
        match message {
          | JournalMessage::WriteMessages { messages, .. } => messages.first().cloned(),
          | _ => None,
        }
      });
    maybe_repr.expect("write message not found")
  };

  let instance_id = actor.context.instance_id();
  actor.handle_journal_response(&JournalResponse::WriteMessageSuccess { repr, instance_id });
  assert_eq!(actor.handled, vec![1_i32]);

  actor.handle_journal_response(&JournalResponse::WriteMessagesSuccessful { instance_id });
  assert_eq!(actor.handled, vec![1_i32, 2_i32]);
}

#[test]
fn persistent_actor_defer_async_without_persistence_runs_immediately() {
  let mut ctx = build_context();
  let mut actor = DummyPersistentActor::new();

  actor.defer_async(&mut ctx, 3_i32, |actor, event| {
    actor.handled.push(*event);
  });

  assert_eq!(actor.handled, vec![3_i32]);
}

#[test]
#[should_panic(
  expected = "Cannot defer during replay. Events can be deferred when receiving RecoveryCompleted or later."
)]
fn persistent_actor_defer_panics_during_recovery_started() {
  let mut ctx = build_context();
  let mut actor = DummyPersistentActor::new();
  start_recovery_without_snapshot(&mut actor);

  actor.defer(&mut ctx, 4_i32, |_actor, _event| {});
}

#[test]
#[should_panic(
  expected = "Cannot defer during replay. Events can be deferred when receiving RecoveryCompleted or later."
)]
fn persistent_actor_defer_async_panics_during_recovering() {
  let mut ctx = build_context();
  let mut actor = DummyPersistentActor::new();
  move_to_recovering(&mut actor);

  actor.defer_async(&mut ctx, 5_i32, |_actor, _event| {});
}
