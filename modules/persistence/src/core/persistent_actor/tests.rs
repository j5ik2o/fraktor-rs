use fraktor_actor_rs::core::{
  actor::{
    Actor, ActorCellGeneric, ActorContextGeneric, Pid,
    actor_ref::{ActorRef, ActorRefGeneric, ActorRefSender, SendOutcome},
  },
  error::{ActorError, SendError},
  messaging::{AnyMessageGeneric, AnyMessageViewGeneric},
  props::PropsGeneric,
  system::{
    ActorSystemGeneric,
    state::{SystemStateSharedGeneric, system_state::SystemStateGeneric},
  },
};
use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdToolbox, RuntimeToolbox, ToolboxMutex, sync_mutex_family::SyncMutexFamily},
  sync::ArcShared,
};

use crate::core::{
  eventsourced::Eventsourced, journal_message::JournalMessage, persistence_context::PersistenceContext,
  persistent_actor::PersistentActor, persistent_repr::PersistentRepr, snapshot::Snapshot,
  snapshot_message::SnapshotMessage, snapshot_selection_criteria::SnapshotSelectionCriteria,
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

struct NoopActor;

impl Actor<TB> for NoopActor {
  fn receive(
    &mut self,
    _ctx: &mut ActorContextGeneric<'_, TB>,
    _message: AnyMessageViewGeneric<'_, TB>,
  ) -> Result<(), ActorError> {
    Ok(())
  }
}

fn build_context() -> ActorContextGeneric<'static, TB> {
  let state = SystemStateSharedGeneric::new(SystemStateGeneric::new());
  let system = ActorSystemGeneric::<TB>::from_state(state.clone());
  let pid = system.allocate_pid();
  let props = PropsGeneric::from_fn(|| NoopActor);
  let cell =
    ActorCellGeneric::create(state.clone(), pid, None, "test".into(), &props).expect("actor cell should be created");
  state.register_cell(cell);
  ActorContextGeneric::new(&system, pid)
}

struct DummyPersistentActor {
  context: PersistenceContext<DummyPersistentActor, NoStdToolbox>,
}

impl DummyPersistentActor {
  fn new() -> Self {
    Self { context: PersistenceContext::new("pid-1".into()) }
  }

  fn new_with_refs(journal: ActorRef, snapshot: ActorRef) -> Self {
    let mut actor = Self::new();
    let _ = actor.context.bind_actor_refs(journal, snapshot);
    actor
  }
}

impl Eventsourced<NoStdToolbox> for DummyPersistentActor {
  fn persistence_id(&self) -> &str {
    self.context.persistence_id()
  }

  fn receive_recover(&mut self, _event: &PersistentRepr) {}

  fn receive_snapshot(&mut self, _snapshot: &Snapshot) {}

  fn receive_command(
    &mut self,
    _ctx: &mut ActorContextGeneric<'_, NoStdToolbox>,
    _message: AnyMessageViewGeneric<'_, NoStdToolbox>,
  ) -> Result<(), ActorError> {
    Ok(())
  }

  fn last_sequence_nr(&self) -> u64 {
    self.context.last_sequence_nr()
  }
}

impl PersistentActor<NoStdToolbox> for DummyPersistentActor {
  fn persistence_context(&mut self) -> &mut PersistenceContext<Self, NoStdToolbox> {
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
fn persistent_actor_save_snapshot_sends_message() {
  let (journal_ref, _journal_store) = create_sender();
  let (snapshot_ref, snapshot_store) = create_sender();
  let mut ctx = build_context();
  let mut actor = DummyPersistentActor::new_with_refs(journal_ref, snapshot_ref);

  let snapshot: ArcShared<dyn core::any::Any + Send + Sync> = ArcShared::new(123_u32);
  actor.save_snapshot(&mut ctx, snapshot);

  let messages = snapshot_store.lock();
  assert_eq!(messages.len(), 1);
  let message = messages[0].payload().downcast_ref::<SnapshotMessage<TB>>().expect("unexpected payload");
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

  actor.delete_messages(&mut ctx, 10);

  let messages = journal_store.lock();
  assert_eq!(messages.len(), 1);
  let message = messages[0].payload().downcast_ref::<JournalMessage<TB>>().expect("unexpected payload");
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
  actor.delete_snapshots(&mut ctx, criteria.clone());

  let messages = snapshot_store.lock();
  assert_eq!(messages.len(), 1);
  let message = messages[0].payload().downcast_ref::<SnapshotMessage<TB>>().expect("unexpected payload");
  match message {
    | SnapshotMessage::DeleteSnapshots { persistence_id, criteria: sent, .. } => {
      assert_eq!(persistence_id, "pid-1");
      assert_eq!(sent, &criteria);
    },
    | _ => panic!("unexpected message"),
  }
}
