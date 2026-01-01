//! Persistent actor flow integration tests.

extern crate alloc;

use alloc::{string::String, vec::Vec};

use fraktor_actor_rs::core::{
  actor::{
    Actor, ActorCellGeneric, ActorContextGeneric, Pid,
    actor_ref::{ActorRefGeneric, ActorRefSender, SendOutcome},
  },
  error::{ActorError, SendError},
  messaging::{AnyMessageGeneric, AnyMessageViewGeneric},
  props::PropsGeneric,
  system::{ActorSystemGeneric, SystemStateGeneric, SystemStateSharedGeneric},
};
use fraktor_persistence_rs::core::{
  Eventsourced, JournalMessage, JournalResponse, PersistentActor, PersistentActorBase, PersistentRepr, Recovery,
  Snapshot, SnapshotMessage, SnapshotMetadata, SnapshotResponse,
};
use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdToolbox, RuntimeToolbox, SyncMutexFamily, ToolboxMutex},
  sync::ArcShared,
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

#[derive(Clone)]
enum Event {
  Incremented(i32),
}

struct TestActor {
  value:             i32,
  recovered:         Vec<i32>,
  recovery_complete: bool,
  base:              PersistentActorBase<TestActor, TB>,
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

impl TestActor {
  fn new(persistence_id: &str, journal: ActorRefGeneric<TB>, snapshot: ActorRefGeneric<TB>) -> Self {
    Self {
      value:             0,
      recovered:         Vec::new(),
      recovery_complete: false,
      base:              PersistentActorBase::new(persistence_id.into(), journal, snapshot),
    }
  }
}

impl Eventsourced<TB> for TestActor {
  fn persistence_id(&self) -> &str {
    self.base.persistence_id()
  }

  fn journal_actor_ref(&self) -> &ActorRefGeneric<TB> {
    self.base.journal_actor_ref()
  }

  fn snapshot_actor_ref(&self) -> &ActorRefGeneric<TB> {
    self.base.snapshot_actor_ref()
  }

  fn receive_recover(&mut self, event: &PersistentRepr) {
    if let Some(event) = event.downcast_ref::<Event>() {
      let Event::Incremented(delta) = event;
      self.value += delta;
      self.recovered.push(*delta);
    }
  }

  fn receive_snapshot(&mut self, snapshot: &Snapshot) {
    if let Some(value) = snapshot.data().downcast_ref::<i32>() {
      self.value = *value;
    }
  }

  fn receive_command(
    &mut self,
    _ctx: &mut ActorContextGeneric<'_, TB>,
    _message: AnyMessageViewGeneric<'_, TB>,
  ) -> Result<(), ActorError> {
    Ok(())
  }

  fn on_recovery_completed(&mut self) {
    self.recovery_complete = true;
  }

  fn last_sequence_nr(&self) -> u64 {
    self.base.last_sequence_nr()
  }

  fn recovery(&self) -> Recovery {
    Recovery::default()
  }
}

impl PersistentActor<TB> for TestActor {
  fn base(&self) -> &PersistentActorBase<Self, TB> {
    &self.base
  }

  fn base_mut(&mut self) -> &mut PersistentActorBase<Self, TB> {
    &mut self.base
  }
}

#[test]
fn recovery_flow_snapshot_then_replay() {
  let (journal_ref, journal_store) = create_sender();
  let (snapshot_ref, snapshot_store) = create_sender();
  let mut actor = TestActor::new("pid-1", journal_ref, snapshot_ref);

  let system = ActorSystemGeneric::from_state(SystemStateSharedGeneric::new(SystemStateGeneric::new()));
  let pid = Pid::new(1, 1);
  let props = PropsGeneric::from_fn(|| NoopActor);
  let cell =
    ActorCellGeneric::create(system.state(), pid, None, String::from("self"), &props).expect("create actor cell");
  system.state().register_cell(cell);
  let mut ctx = ActorContextGeneric::new(&system, pid);
  actor.start_recovery(&mut ctx);

  let snapshot_messages = snapshot_store.lock();
  assert_eq!(snapshot_messages.len(), 1);
  let load = snapshot_messages[0].payload().downcast_ref::<SnapshotMessage<TB>>().expect("snapshot message");
  assert!(matches!(load, SnapshotMessage::LoadSnapshot { .. }));

  let snapshot = Snapshot::new(SnapshotMetadata::new("pid-1", 2, 0), ArcShared::new(10_i32));
  let response = SnapshotResponse::LoadSnapshotResult { snapshot: Some(snapshot), to_sequence_nr: 2 };
  actor.handle_snapshot_response(&response, &mut ctx);

  let journal_messages = journal_store.lock();
  assert_eq!(journal_messages.len(), 1);
  let replay = journal_messages[0].payload().downcast_ref::<JournalMessage<TB>>().expect("journal message");
  assert!(matches!(replay, JournalMessage::ReplayMessages { .. }));

  let repr1 = PersistentRepr::new("pid-1", 3, ArcShared::new(Event::Incremented(2)));
  let repr2 = PersistentRepr::new("pid-1", 4, ArcShared::new(Event::Incremented(3)));
  actor.handle_journal_response(&JournalResponse::ReplayedMessage { persistent_repr: repr1 });
  actor.handle_journal_response(&JournalResponse::ReplayedMessage { persistent_repr: repr2 });
  actor.handle_journal_response(&JournalResponse::RecoverySuccess { highest_sequence_nr: 4 });

  assert!(actor.recovery_complete);
  assert_eq!(actor.value, 15);
  assert_eq!(actor.recovered, vec![2, 3]);
}

#[test]
fn persist_flow_sends_write_messages() {
  let (journal_ref, journal_store) = create_sender();
  let (snapshot_ref, _snapshot_store) = create_sender();
  let mut actor = TestActor::new("pid-1", journal_ref, snapshot_ref);

  let mut ctx = ActorContextGeneric::new(
    &ActorSystemGeneric::from_state(SystemStateSharedGeneric::new(SystemStateGeneric::new())),
    Pid::new(1, 1),
  );

  actor.persist(&mut ctx, Event::Incremented(1), |_actor, _| {});
  actor.base.flush_batch(ActorRefGeneric::null());

  let journal_messages = journal_store.lock();
  assert_eq!(journal_messages.len(), 1);
  let write = journal_messages[0].payload().downcast_ref::<JournalMessage<TB>>().expect("journal message");
  assert!(matches!(write, JournalMessage::WriteMessages { .. }));
}
