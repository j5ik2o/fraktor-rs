use alloc::vec::Vec;

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
  eventsourced::Eventsourced, journal_message::JournalMessage, journal_response::JournalResponse,
  persistence_context::PersistenceContext, persistent_actor::PersistentActor,
  persistent_actor_state::PersistentActorState, persistent_fsm::PersistentFsm, persistent_repr::PersistentRepr,
  recovery::Recovery, snapshot::Snapshot, snapshot_response::SnapshotResponse,
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

fn create_sender_with_pid(pid: Pid) -> (ActorRefGeneric<TB>, MessageStore) {
  let messages = ArcShared::new(<<NoStdToolbox as RuntimeToolbox>::MutexFamily as SyncMutexFamily>::create(Vec::new()));
  let sender = ActorRefGeneric::new(pid, TestSender { messages: messages.clone() });
  (sender, messages)
}

fn create_sender() -> (ActorRefGeneric<TB>, MessageStore) {
  create_sender_with_pid(Pid::new(1, 1))
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TestFsmState {
  Ready,
  Updated,
}

struct TestFsmEvent {
  increment: i32,
}

struct TestPersistentFsmActor {
  context: PersistenceContext<TestPersistentFsmActor, TB>,
  state:   TestFsmState,
  total:   i32,
}

impl TestPersistentFsmActor {
  fn new() -> Self {
    Self { context: PersistenceContext::new("pid-1".into()), state: TestFsmState::Ready, total: 0 }
  }

  fn new_with_refs(journal: ActorRef, snapshot: ActorRef) -> Self {
    let mut actor = Self::new();
    let _ = actor.context.bind_actor_refs(journal, snapshot);
    actor
  }
}

impl Eventsourced<TB> for TestPersistentFsmActor {
  fn persistence_id(&self) -> &str {
    self.context.persistence_id()
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
    self.context.last_sequence_nr()
  }
}

impl PersistentActor<TB> for TestPersistentFsmActor {
  fn persistence_context(&mut self) -> &mut PersistenceContext<Self, TB> {
    &mut self.context
  }
}

impl PersistentFsm<TB> for TestPersistentFsmActor {
  type DomainEvent = TestFsmEvent;
  type State = TestFsmState;

  fn apply_fsm_event(&mut self, event: &Self::DomainEvent) {
    self.total += event.increment;
  }

  fn set_fsm_state(&mut self, state: Self::State) {
    self.state = state;
  }

  fn fsm_state(&self) -> &Self::State {
    &self.state
  }
}

fn move_to_processing_commands(actor: &mut TestPersistentFsmActor) {
  actor.context.start_recovery(Recovery::default(), ActorRefGeneric::null()).expect("start recovery");
  let _ = actor.context.handle_snapshot_response(
    &SnapshotResponse::LoadSnapshotResult { snapshot: None, to_sequence_nr: u64::MAX },
    ActorRefGeneric::null(),
  );
  let _ = actor.context.handle_journal_response(&JournalResponse::RecoverySuccess { highest_sequence_nr: 0 });
}

fn first_write_message_repr(journal_store: &MessageStore) -> PersistentRepr {
  let messages = journal_store.lock();
  messages
    .iter()
    .filter_map(|message| message.payload().downcast_ref::<JournalMessage<TB>>())
    .find_map(|message| match message {
      | JournalMessage::WriteMessages { messages, .. } => messages.first().cloned(),
      | _ => None,
    })
    .expect("write message not found")
}

#[test]
fn persist_state_transition_applies_event_and_state_after_ack() {
  let (journal_ref, journal_store) = create_sender();
  let (snapshot_ref, _snapshot_store) = create_sender();
  let mut ctx = build_context();
  let mut actor = TestPersistentFsmActor::new_with_refs(journal_ref, snapshot_ref);
  move_to_processing_commands(&mut actor);

  actor.persist_state_transition(&mut ctx, TestFsmEvent { increment: 3 }, TestFsmState::Updated);
  actor.flush_batch(&mut ctx).expect("flush");

  assert_eq!(actor.fsm_state(), &TestFsmState::Ready);
  assert_eq!(actor.total, 0);
  assert!(actor.context.should_stash_commands());

  let repr = first_write_message_repr(&journal_store);
  let instance_id = actor.context.instance_id();
  actor.handle_journal_response(&JournalResponse::WriteMessageSuccess { repr, instance_id });
  actor.handle_journal_response(&JournalResponse::WriteMessagesSuccessful { instance_id });

  assert_eq!(actor.fsm_state(), &TestFsmState::Updated);
  assert_eq!(actor.total, 3);
  assert_eq!(actor.context.state(), PersistentActorState::ProcessingCommands);
}

#[test]
fn persist_state_transition_async_does_not_enable_stash_commands() {
  let (journal_ref, journal_store) = create_sender();
  let (snapshot_ref, _snapshot_store) = create_sender();
  let mut ctx = build_context();
  let mut actor = TestPersistentFsmActor::new_with_refs(journal_ref, snapshot_ref);
  move_to_processing_commands(&mut actor);

  actor.persist_state_transition_async(&mut ctx, TestFsmEvent { increment: 4 }, TestFsmState::Updated);
  actor.flush_batch(&mut ctx).expect("flush");

  assert!(!actor.context.should_stash_commands());

  let repr = first_write_message_repr(&journal_store);
  let instance_id = actor.context.instance_id();
  actor.handle_journal_response(&JournalResponse::WriteMessageSuccess { repr, instance_id });
  actor.handle_journal_response(&JournalResponse::WriteMessagesSuccessful { instance_id });

  assert_eq!(actor.fsm_state(), &TestFsmState::Updated);
  assert_eq!(actor.total, 4);
}
