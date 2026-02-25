use alloc::{
  boxed::Box,
  string::{String, ToString},
  vec,
  vec::Vec,
};
use core::any::{Any, TypeId};

use fraktor_actor_rs::core::{
  actor::{
    ActorContextGeneric, Pid,
    actor_ref::{ActorRefGeneric, ActorRefSender, SendOutcome},
  },
  error::{ActorError, SendError},
  messaging::{AnyMessageGeneric, AnyMessageViewGeneric},
};
use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdToolbox, RuntimeToolbox, ToolboxMutex, sync_mutex_family::SyncMutexFamily},
  sync::ArcShared,
};

use crate::core::{
  event_adapters::EventAdapters, event_seq::EventSeq, eventsourced::Eventsourced, journal_message::JournalMessage,
  journal_response::JournalResponse, journal_response_action::JournalResponseAction,
  persistence_context::PersistenceContext, persistent_actor_state::PersistentActorState,
  persistent_repr::PersistentRepr, read_event_adapter::ReadEventAdapter, snapshot::Snapshot,
  snapshot_message::SnapshotMessage, write_event_adapter::WriteEventAdapter,
};

type TB = NoStdToolbox;
type MessageStore = ArcShared<ToolboxMutex<Vec<AnyMessageGeneric<TB>>, TB>>;
const ADD_TEN_MANIFEST: &str = "add-ten-v1";
const SINGLE_MANIFEST: &str = "single-v1";
const EMPTY_MANIFEST: &str = "empty-v1";

#[derive(Default)]
struct DummyActor {
  handled_values:   Vec<i32>,
  recovered_values: Vec<i32>,
}

type DummyContext = PersistenceContext<DummyActor, TB>;

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
  let messages = ArcShared::new(<<TB as RuntimeToolbox>::MutexFamily as SyncMutexFamily>::create(Vec::new()));
  let sender = ActorRefGeneric::new(Pid::new(1, 1), TestSender { messages: messages.clone() });
  (sender, messages)
}

struct AddTenWriteAdapter;

impl WriteEventAdapter for AddTenWriteAdapter {
  fn manifest(&self, _event: &(dyn Any + Send + Sync)) -> String {
    ADD_TEN_MANIFEST.to_string()
  }

  fn to_journal(&self, event: ArcShared<dyn Any + Send + Sync>) -> ArcShared<dyn Any + Send + Sync> {
    let value = event.downcast_ref::<i32>().expect("expected i32 event");
    ArcShared::new((*value + 10).to_string())
  }
}

struct SplitReadAdapter;

impl ReadEventAdapter for SplitReadAdapter {
  fn adapt_from_journal(&self, event: ArcShared<dyn Any + Send + Sync>, manifest: &str) -> EventSeq {
    if manifest != ADD_TEN_MANIFEST && manifest != SINGLE_MANIFEST && manifest != EMPTY_MANIFEST {
      return EventSeq::single(event);
    }
    let value = event.downcast_ref::<String>().expect("expected string event");
    let value = value.parse::<i32>().expect("expected numeric string");
    if manifest == ADD_TEN_MANIFEST {
      return EventSeq::multiple(vec![ArcShared::new(value), ArcShared::new(value + 1)]);
    }
    if manifest == SINGLE_MANIFEST {
      return EventSeq::single(ArcShared::new(value));
    }
    EventSeq::empty()
  }
}

impl Eventsourced<TB> for DummyActor {
  fn persistence_id(&self) -> &str {
    "pid-1"
  }

  fn receive_recover(&mut self, event: &PersistentRepr) {
    let value = event.downcast_ref::<i32>().expect("expected i32 replay event");
    self.recovered_values.push(*value);
  }

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
}

fn replay_persistent_repr(sequence_nr: u64, value: i32, manifest: &str) -> PersistentRepr {
  let write_adapter: ArcShared<dyn WriteEventAdapter> = ArcShared::new(AddTenWriteAdapter);
  let read_adapter: ArcShared<dyn ReadEventAdapter> = ArcShared::new(SplitReadAdapter);
  let mut adapters = EventAdapters::new();
  adapters.register::<i32>(write_adapter, read_adapter);
  PersistentRepr::new("pid-1", sequence_nr, ArcShared::new(value.to_string()))
    .with_manifest(manifest)
    .with_adapters(adapters)
    .with_adapter_type_id(TypeId::of::<i32>())
}

#[test]
fn context_sends_journal_messages() {
  let (journal_ref, journal_store) = create_sender();
  let (snapshot_ref, _snapshot_store) = create_sender();
  let mut context = DummyContext::new("pid-1".to_string());
  context.bind_actor_refs(journal_ref, snapshot_ref).expect("bind actor refs");

  let message = JournalMessage::DeleteMessagesTo {
    persistence_id: "pid-1".to_string(),
    to_sequence_nr: 10,
    sender:         ActorRefGeneric::null(),
  };
  context.send_write_messages(message).expect("send");

  let messages = journal_store.lock();
  assert_eq!(messages.len(), 1);
}

#[test]
fn context_sends_snapshot_messages() {
  let (journal_ref, _journal_store) = create_sender();
  let (snapshot_ref, snapshot_store) = create_sender();
  let mut context = DummyContext::new("pid-1".to_string());
  context.bind_actor_refs(journal_ref, snapshot_ref).expect("bind actor refs");

  let message = SnapshotMessage::DeleteSnapshots {
    persistence_id: "pid-1".to_string(),
    criteria:       crate::core::snapshot_selection_criteria::SnapshotSelectionCriteria::latest(),
    sender:         ActorRefGeneric::null(),
  };
  context.send_snapshot_message(message).expect("send");

  let messages = snapshot_store.lock();
  assert_eq!(messages.len(), 1);
}

#[test]
fn bind_actor_refs_rejects_second_bind() {
  let (journal_ref, _journal_store) = create_sender();
  let (snapshot_ref, _snapshot_store) = create_sender();
  let mut context = DummyContext::new("pid-1".to_string());

  assert!(context.bind_actor_refs(journal_ref, snapshot_ref).is_ok());
  let result = context.bind_actor_refs(ActorRefGeneric::null(), ActorRefGeneric::null());
  assert!(result.is_err());
}

/// Regression test for GitHub Issue #125: `is_bound` must use OR semantics
/// (either ref non-null → bound) while `is_ready` uses AND semantics
/// (both refs non-null → ready). PR #117 accidentally changed `is_bound`
/// to AND, making it identical to `is_ready`.
#[test]
fn is_bound_returns_true_when_only_journal_ref_is_set() {
  // Given: a context with only journal_actor_ref set (partial binding)
  let mut context = DummyContext::new("pid-1".to_string());
  let (journal_ref, _) = create_sender();
  context.journal_actor_ref = journal_ref;

  // Then: is_bound is true (OR: at least one ref is set)
  assert!(context.is_bound());
  // Then: is_ready is false (AND: both refs must be set)
  assert!(!context.is_ready());
}

/// Regression test for GitHub Issue #125: partial binding with only
/// snapshot_actor_ref must also be considered "bound".
#[test]
fn is_bound_returns_true_when_only_snapshot_ref_is_set() {
  // Given: a context with only snapshot_actor_ref set (partial binding)
  let mut context = DummyContext::new("pid-1".to_string());
  let (snapshot_ref, _) = create_sender();
  context.snapshot_actor_ref = snapshot_ref;

  // Then: is_bound is true (OR: at least one ref is set)
  assert!(context.is_bound());
  // Then: is_ready is false (AND: both refs must be set)
  assert!(!context.is_ready());
}

#[test]
fn is_bound_returns_false_when_neither_ref_is_set() {
  // Given: a freshly created context (both refs are null)
  let context = DummyContext::new("pid-1".to_string());

  // Then: neither bound nor ready
  assert!(!context.is_bound());
  assert!(!context.is_ready());
}

#[test]
fn start_recovery_none_requests_highest_sequence_nr() {
  let (journal_ref, journal_store) = create_sender();
  let (snapshot_ref, snapshot_store) = create_sender();
  let mut context = DummyContext::new("pid-1".to_string());
  context.bind_actor_refs(journal_ref, snapshot_ref).expect("bind actor refs");

  context.start_recovery(crate::core::Recovery::none(), ActorRefGeneric::null()).expect("start recovery");

  let journal_messages = journal_store.lock();
  assert_eq!(journal_messages.len(), 1);
  let message = journal_messages[0].payload().downcast_ref::<JournalMessage<TB>>().expect("unexpected payload");
  match message {
    | JournalMessage::GetHighestSequenceNr { persistence_id, from_sequence_nr, .. } => {
      assert_eq!(persistence_id, "pid-1");
      assert_eq!(*from_sequence_nr, 0);
    },
    | _ => panic!("unexpected message"),
  }

  let snapshot_messages = snapshot_store.lock();
  assert_eq!(snapshot_messages.len(), 0);
}

#[test]
fn context_applies_event_adapters_on_persist_and_replay() {
  let (journal_ref, journal_store) = create_sender();
  let (snapshot_ref, _snapshot_store) = create_sender();
  let mut context = DummyContext::new("pid-1".to_string());
  context.bind_actor_refs(journal_ref, snapshot_ref).expect("bind actor refs");
  context.state = PersistentActorState::ProcessingCommands;

  let write_adapter: ArcShared<dyn WriteEventAdapter> = ArcShared::new(AddTenWriteAdapter);
  let read_adapter: ArcShared<dyn ReadEventAdapter> = ArcShared::new(SplitReadAdapter);
  context.event_adapters_mut().register::<i32>(write_adapter, read_adapter);

  context.add_to_event_batch(
    5_i32,
    true,
    Box::new(|actor: &mut DummyActor, repr| {
      let value = repr.downcast_ref::<i32>().expect("expected i32 event");
      actor.handled_values.push(*value);
    }),
  );
  context.flush_batch(ActorRefGeneric::null()).expect("flush batch");

  let persisted_repr = {
    let journal_messages = journal_store.lock();
    assert_eq!(journal_messages.len(), 1);
    let message = journal_messages[0].payload().downcast_ref::<JournalMessage<TB>>().expect("unexpected payload");
    match message {
      | JournalMessage::WriteMessages { messages, .. } => {
        assert_eq!(messages.len(), 1);
        messages[0].clone()
      },
      | _ => panic!("unexpected message"),
    }
  };

  assert_eq!(persisted_repr.manifest(), ADD_TEN_MANIFEST);
  assert_eq!(persisted_repr.downcast_ref::<String>().map(String::as_str), Some("15"));

  let write_action = context.handle_journal_response(&JournalResponse::WriteMessageSuccess {
    repr:        persisted_repr.clone(),
    instance_id: 1,
  });
  let mut actor = DummyActor::default();
  match write_action {
    | JournalResponseAction::InvokeHandler(invocation) => invocation.invoke(&mut actor),
    | _ => panic!("expected invoke handler"),
  }
  assert_eq!(actor.handled_values, vec![5_i32]);

  let replay_action =
    context.handle_journal_response(&JournalResponse::ReplayedMessage { persistent_repr: persisted_repr.clone() });
  let mut recovering_actor = DummyActor::default();
  replay_action.apply::<TB>(&mut recovering_actor);
  assert_eq!(recovering_actor.recovered_values, vec![15_i32, 16_i32]);
}

#[test]
fn replayed_message_returns_receive_recover_for_single_event() {
  let mut context = DummyContext::new("pid-1".to_string());
  let replay_repr = replay_persistent_repr(3, 21, SINGLE_MANIFEST);

  let replay_action =
    context.handle_journal_response(&JournalResponse::ReplayedMessage { persistent_repr: replay_repr });

  match replay_action {
    | JournalResponseAction::ReceiveRecover(repr) => {
      assert_eq!(repr.downcast_ref::<i32>(), Some(&21_i32));
    },
    | _ => panic!("expected single replay message"),
  }
}

#[test]
fn replayed_message_returns_none_for_empty_event_seq() {
  let mut context = DummyContext::new("pid-1".to_string());
  let replay_repr = replay_persistent_repr(4, 21, EMPTY_MANIFEST);

  let replay_action =
    context.handle_journal_response(&JournalResponse::ReplayedMessage { persistent_repr: replay_repr });

  match replay_action {
    | JournalResponseAction::None => {},
    | _ => panic!("expected no replay action"),
  }
  assert_eq!(context.current_sequence_nr(), 4);
}
