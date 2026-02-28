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
  event_adapters::EventAdapters, event_seq::EventSeq, eventsourced::Eventsourced, journal_error::JournalError,
  journal_message::JournalMessage, journal_response::JournalResponse, journal_response_action::JournalResponseAction,
  pending_handler_invocation::PendingHandlerInvocation, persistence_context::PersistenceContext,
  persistent_actor_state::PersistentActorState, persistent_repr::PersistentRepr, read_event_adapter::ReadEventAdapter,
  snapshot::Snapshot, snapshot_message::SnapshotMessage, write_event_adapter::WriteEventAdapter,
};

type TB = NoStdToolbox;
type MessageStore = ArcShared<ToolboxMutex<Vec<AnyMessageGeneric<TB>>, TB>>;
type DummyPendingHandler = Box<dyn FnOnce(&mut DummyActor, &PersistentRepr) + Send + Sync>;
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

struct FailingSender;

impl ActorRefSender<TB> for FailingSender {
  fn send(&mut self, message: AnyMessageGeneric<TB>) -> Result<SendOutcome, SendError<TB>> {
    Err(SendError::closed(message))
  }
}

fn create_failing_sender() -> ActorRefGeneric<TB> {
  ActorRefGeneric::new(Pid::new(2, 1), FailingSender)
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
fn context_assigns_unique_instance_id_per_context() {
  let first = DummyContext::new("pid-1".to_string()).instance_id();
  let second = DummyContext::new("pid-1".to_string()).instance_id();

  assert_ne!(first, second);
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
fn highest_sequence_nr_completes_recovery_none_path() {
  let (journal_ref, _journal_store) = create_sender();
  let (snapshot_ref, _snapshot_store) = create_sender();
  let mut context = DummyContext::new("pid-1".to_string());
  context.bind_actor_refs(journal_ref, snapshot_ref).expect("bind actor refs");
  context.start_recovery(crate::core::Recovery::none(), ActorRefGeneric::null()).expect("start recovery");
  assert_eq!(context.state(), PersistentActorState::RecoveryStarted);

  let action = context.handle_journal_response(&JournalResponse::HighestSequenceNr {
    persistence_id: "pid-1".to_string(),
    sequence_nr:    42,
  });

  match action {
    | JournalResponseAction::RecoveryCompleted => {},
    | _ => panic!("expected recovery completion"),
  }
  assert_eq!(context.state(), PersistentActorState::ProcessingCommands);
  assert_eq!(context.current_sequence_nr(), 42);
  assert_eq!(context.last_sequence_nr(), 42);
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
    None,
    Box::new(|actor: &mut DummyActor, repr| {
      let value = repr.downcast_ref::<i32>().expect("expected i32 event");
      actor.handled_values.push(*value);
    }),
  );
  context.flush_batch(ActorRefGeneric::null()).expect("flush batch");
  let expected_instance_id = context.instance_id();

  let persisted_repr = {
    let journal_messages = journal_store.lock();
    assert_eq!(journal_messages.len(), 1);
    let message = journal_messages[0].payload().downcast_ref::<JournalMessage<TB>>().expect("unexpected payload");
    match message {
      | JournalMessage::WriteMessages { messages, instance_id, .. } => {
        assert_eq!(*instance_id, expected_instance_id);
        assert_eq!(messages.len(), 1);
        messages[0].clone()
      },
      | _ => panic!("unexpected message"),
    }
  };

  assert_eq!(persisted_repr.manifest(), ADD_TEN_MANIFEST);
  assert_eq!(persisted_repr.downcast_ref::<String>().map(String::as_str), Some("15"));
  let instance_id = context.instance_id();

  let write_action = context
    .handle_journal_response(&JournalResponse::WriteMessageSuccess { repr: persisted_repr.clone(), instance_id });
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

#[test]
fn write_messages_successful_triggers_deferred_handlers() {
  let (journal_ref, journal_store) = create_sender();
  let (snapshot_ref, _snapshot_store) = create_sender();
  let mut context = DummyContext::new("pid-1".to_string());
  context.bind_actor_refs(journal_ref, snapshot_ref).expect("bind actor refs");
  context.state = PersistentActorState::ProcessingCommands;

  context.add_to_event_batch(
    1_i32,
    true,
    None,
    Box::new(|actor: &mut DummyActor, repr| {
      let value = repr.downcast_ref::<i32>().expect("expected i32 event");
      actor.handled_values.push(*value);
    }),
  );
  context.add_deferred_handler(
    2_i32,
    true,
    None,
    Box::new(|actor: &mut DummyActor, repr| {
      let value = repr.downcast_ref::<i32>().expect("expected i32 event");
      actor.handled_values.push(*value);
    }),
  );
  context.flush_batch(ActorRefGeneric::null()).expect("flush batch");

  let persisted_repr = {
    let journal_messages = journal_store.lock();
    let message = journal_messages[0].payload().downcast_ref::<JournalMessage<TB>>().expect("unexpected payload");
    match message {
      | JournalMessage::WriteMessages { messages, .. } => messages[0].clone(),
      | _ => panic!("unexpected message"),
    }
  };

  let instance_id = context.instance_id();
  let mut actor = DummyActor::default();
  let write_action =
    context.handle_journal_response(&JournalResponse::WriteMessageSuccess { repr: persisted_repr, instance_id });
  write_action.apply::<TB>(&mut actor);
  assert_eq!(actor.handled_values, vec![1_i32]);

  let deferred_action = context.handle_journal_response(&JournalResponse::WriteMessagesSuccessful { instance_id });
  deferred_action.apply::<TB>(&mut actor);
  assert_eq!(actor.handled_values, vec![1_i32, 2_i32]);
}

#[test]
fn deferred_handler_repr_keeps_sender() {
  let mut context = DummyContext::new("pid-1".to_string());
  context.state = PersistentActorState::PersistingEvents;
  let sender = Pid::new(7, 1);

  context.add_deferred_handler(
    2_i32,
    false,
    Some(sender),
    Box::new(move |actor: &mut DummyActor, repr| {
      assert_eq!(repr.sender(), Some(sender));
      let value = repr.downcast_ref::<i32>().expect("expected i32 event");
      actor.handled_values.push(*value);
    }),
  );

  let mut actor = DummyActor::default();
  let action =
    context.handle_journal_response(&JournalResponse::WriteMessagesSuccessful { instance_id: context.instance_id() });
  action.apply::<TB>(&mut actor);

  assert_eq!(actor.handled_values, vec![2_i32]);
  assert_eq!(context.state(), PersistentActorState::ProcessingCommands);
}

#[test]
fn flush_batch_clears_sender_in_journal_repr_but_keeps_it_for_handler_invocation() {
  let (journal_ref, journal_store) = create_sender();
  let (snapshot_ref, _snapshot_store) = create_sender();
  let mut context = DummyContext::new("pid-1".to_string());
  context.bind_actor_refs(journal_ref, snapshot_ref).expect("bind actor refs");
  context.state = PersistentActorState::ProcessingCommands;

  let sender = Pid::new(7, 1);
  context.add_to_event_batch(
    11_i32,
    false,
    Some(sender),
    Box::new(move |actor: &mut DummyActor, repr| {
      assert_eq!(repr.sender(), Some(sender));
      let value = repr.downcast_ref::<i32>().expect("expected i32 event");
      actor.handled_values.push(*value);
    }),
  );
  context.flush_batch(ActorRefGeneric::null()).expect("flush batch");

  let journal_repr = {
    let journal_messages = journal_store.lock();
    let message = journal_messages[0].payload().downcast_ref::<JournalMessage<TB>>().expect("unexpected payload");
    match message {
      | JournalMessage::WriteMessages { messages, .. } => messages[0].clone(),
      | _ => panic!("unexpected message"),
    }
  };
  assert_eq!(journal_repr.sender(), None);

  let mut actor = DummyActor::default();
  let action = context.handle_journal_response(&JournalResponse::WriteMessageSuccess {
    repr:        journal_repr,
    instance_id: context.instance_id(),
  });
  action.apply::<TB>(&mut actor);
  assert_eq!(actor.handled_values, vec![11_i32]);
}

fn assert_handler_not_double_boxed(stashing: bool) {
  let (journal_ref, _journal_store) = create_sender();
  let (snapshot_ref, _snapshot_store) = create_sender();
  let mut context = DummyContext::new("pid-1".to_string());
  context.bind_actor_refs(journal_ref, snapshot_ref).expect("bind actor refs");
  context.state = PersistentActorState::ProcessingCommands;

  let handler: DummyPendingHandler = Box::new(|_actor: &mut DummyActor, _repr| {});
  let original_ptr = (&*handler as *const dyn FnOnce(&mut DummyActor, &PersistentRepr)) as *const ();
  context.add_to_event_batch(1_i32, stashing, None, handler);
  context.flush_batch(ActorRefGeneric::null()).expect("flush batch");

  let invocation = context.pending_invocations.pop_front().expect("pending invocation");
  let stored_ptr = match invocation {
    | PendingHandlerInvocation::Stashing { handler, .. } if stashing => {
      (&*handler as *const dyn FnOnce(&mut DummyActor, &PersistentRepr)) as *const ()
    },
    | PendingHandlerInvocation::Async { handler, .. } if !stashing => {
      (&*handler as *const dyn FnOnce(&mut DummyActor, &PersistentRepr)) as *const ()
    },
    | _ => panic!("unexpected invocation variant"),
  };
  assert_eq!(stored_ptr, original_ptr);
}

#[test]
fn flush_batch_reuses_pre_boxed_stashing_handler_without_double_boxing() {
  assert_handler_not_double_boxed(true);
}

#[test]
fn flush_batch_reuses_pre_boxed_async_handler_without_double_boxing() {
  assert_handler_not_double_boxed(false);
}

#[test]
fn flush_batch_send_failure_rolls_back_and_clears_stash_until_batch_completion() {
  let journal_ref = create_failing_sender();
  let (snapshot_ref, _snapshot_store) = create_sender();
  let mut context = DummyContext::new("pid-1".to_string());
  context.bind_actor_refs(journal_ref, snapshot_ref).expect("bind actor refs");
  context.state = PersistentActorState::ProcessingCommands;

  context.add_to_event_batch(1_i32, true, None, Box::new(|_actor: &mut DummyActor, _repr| {}));
  let result = context.flush_batch(ActorRefGeneric::null());
  assert!(result.is_err());
  assert_eq!(context.state(), PersistentActorState::ProcessingCommands);
  assert!(context.pending_invocations.is_empty());
  assert!(!context.stash_until_batch_completion);
}

#[test]
fn write_message_success_interleaves_defer_between_persisted_handlers() {
  let (journal_ref, journal_store) = create_sender();
  let (snapshot_ref, _snapshot_store) = create_sender();
  let mut context = DummyContext::new("pid-1".to_string());
  context.bind_actor_refs(journal_ref, snapshot_ref).expect("bind actor refs");
  context.state = PersistentActorState::ProcessingCommands;

  context.add_to_event_batch(
    1_i32,
    false,
    None,
    Box::new(|actor: &mut DummyActor, repr| {
      let value = repr.downcast_ref::<i32>().expect("expected i32 event");
      actor.handled_values.push(*value);
    }),
  );
  context.add_deferred_handler(
    2_i32,
    false,
    None,
    Box::new(|actor: &mut DummyActor, repr| {
      let value = repr.downcast_ref::<i32>().expect("expected i32 event");
      actor.handled_values.push(*value);
    }),
  );
  context.add_to_event_batch(
    3_i32,
    false,
    None,
    Box::new(|actor: &mut DummyActor, repr| {
      let value = repr.downcast_ref::<i32>().expect("expected i32 event");
      actor.handled_values.push(*value);
    }),
  );
  context.flush_batch(ActorRefGeneric::null()).expect("flush batch");

  let persisted_reprs = {
    let journal_messages = journal_store.lock();
    let message = journal_messages[0].payload().downcast_ref::<JournalMessage<TB>>().expect("unexpected payload");
    match message {
      | JournalMessage::WriteMessages { to_sequence_nr, messages, .. } => {
        assert_eq!(*to_sequence_nr, 2);
        messages.clone()
      },
      | _ => panic!("unexpected message"),
    }
  };
  assert_eq!(persisted_reprs.len(), 2);

  let instance_id = context.instance_id();
  let mut actor = DummyActor::default();
  let first_action = context
    .handle_journal_response(&JournalResponse::WriteMessageSuccess { repr: persisted_reprs[0].clone(), instance_id });
  first_action.apply::<TB>(&mut actor);
  assert_eq!(actor.handled_values, vec![1_i32]);

  let second_action = context
    .handle_journal_response(&JournalResponse::WriteMessageSuccess { repr: persisted_reprs[1].clone(), instance_id });
  second_action.apply::<TB>(&mut actor);
  assert_eq!(actor.handled_values, vec![1_i32, 2_i32, 3_i32]);

  let completion_action = context.handle_journal_response(&JournalResponse::WriteMessagesSuccessful { instance_id });
  completion_action.apply::<TB>(&mut actor);
  assert_eq!(actor.handled_values, vec![1_i32, 2_i32, 3_i32]);
}

#[test]
fn write_message_success_with_mismatched_instance_id_is_ignored() {
  let (journal_ref, journal_store) = create_sender();
  let (snapshot_ref, _snapshot_store) = create_sender();
  let mut context = DummyContext::new("pid-1".to_string());
  context.bind_actor_refs(journal_ref, snapshot_ref).expect("bind actor refs");
  context.state = PersistentActorState::ProcessingCommands;

  context.add_to_event_batch(
    1_i32,
    true,
    None,
    Box::new(|actor: &mut DummyActor, repr| {
      let value = repr.downcast_ref::<i32>().expect("expected i32 event");
      actor.handled_values.push(*value);
    }),
  );
  context.flush_batch(ActorRefGeneric::null()).expect("flush batch");

  let persisted_repr = {
    let journal_messages = journal_store.lock();
    let message = journal_messages[0].payload().downcast_ref::<JournalMessage<TB>>().expect("unexpected payload");
    match message {
      | JournalMessage::WriteMessages { messages, .. } => messages[0].clone(),
      | _ => panic!("unexpected message"),
    }
  };

  let instance_id = context.instance_id();
  let mismatched_instance_id = instance_id.wrapping_add(1);
  let ignored_action = context.handle_journal_response(&JournalResponse::WriteMessageSuccess {
    repr:        persisted_repr.clone(),
    instance_id: mismatched_instance_id,
  });
  assert!(matches!(ignored_action, JournalResponseAction::None));
  assert_eq!(context.state(), PersistentActorState::PersistingEvents);

  let mut actor = DummyActor::default();
  let success_action =
    context.handle_journal_response(&JournalResponse::WriteMessageSuccess { repr: persisted_repr, instance_id });
  success_action.apply::<TB>(&mut actor);
  assert_eq!(actor.handled_values, vec![1_i32]);
}

#[test]
fn should_stash_commands_only_when_stashing_invocation_exists() {
  let mut context = DummyContext::new("pid-1".to_string());
  context.state = PersistentActorState::PersistingEvents;
  let payload: ArcShared<dyn core::any::Any + Send + Sync> = ArcShared::new(1_i32);
  let repr = PersistentRepr::new("pid-1", 1, payload.clone());

  context.pending_invocations.push_back(PendingHandlerInvocation::async_handler(repr.clone(), |_, _| {}));
  assert!(!context.should_stash_commands());

  context.pending_invocations.clear();
  context.pending_invocations.push_back(PendingHandlerInvocation::stashing(repr, |_, _| {}));
  assert!(context.should_stash_commands());
}

#[test]
fn should_stash_commands_when_stashing_defer_waits_for_batch_success() {
  let (journal_ref, journal_store) = create_sender();
  let (snapshot_ref, _snapshot_store) = create_sender();
  let mut context = DummyContext::new("pid-1".to_string());
  context.bind_actor_refs(journal_ref, snapshot_ref).expect("bind actor refs");
  context.state = PersistentActorState::ProcessingCommands;

  context.add_to_event_batch(1_i32, false, None, Box::new(|_actor: &mut DummyActor, _repr| {}));
  context.add_deferred_handler(2_i32, true, None, Box::new(|_actor: &mut DummyActor, _repr| {}));
  context.flush_batch(ActorRefGeneric::null()).expect("flush batch");

  let persisted_repr = {
    let journal_messages = journal_store.lock();
    let message = journal_messages[0].payload().downcast_ref::<JournalMessage<TB>>().expect("unexpected payload");
    match message {
      | JournalMessage::WriteMessages { messages, .. } => messages[0].clone(),
      | _ => panic!("unexpected message"),
    }
  };

  let instance_id = context.instance_id();
  let _ = context.handle_journal_response(&JournalResponse::WriteMessageSuccess { repr: persisted_repr, instance_id });
  assert_eq!(context.state(), PersistentActorState::PersistingEvents);
  assert!(context.should_stash_commands());

  let _ = context.handle_journal_response(&JournalResponse::WriteMessagesSuccessful { instance_id });
  assert!(!context.should_stash_commands());
}

#[test]
fn should_stash_commands_until_write_messages_successful_for_stashing_batch() {
  let (journal_ref, journal_store) = create_sender();
  let (snapshot_ref, _snapshot_store) = create_sender();
  let mut context = DummyContext::new("pid-1".to_string());
  context.bind_actor_refs(journal_ref, snapshot_ref).expect("bind actor refs");
  context.state = PersistentActorState::ProcessingCommands;

  context.add_to_event_batch(1_i32, true, None, Box::new(|_actor: &mut DummyActor, _repr| {}));
  context.flush_batch(ActorRefGeneric::null()).expect("flush batch");

  let persisted_repr = {
    let journal_messages = journal_store.lock();
    let message = journal_messages[0].payload().downcast_ref::<JournalMessage<TB>>().expect("unexpected payload");
    match message {
      | JournalMessage::WriteMessages { messages, .. } => messages[0].clone(),
      | _ => panic!("unexpected message"),
    }
  };

  let instance_id = context.instance_id();
  let _ = context.handle_journal_response(&JournalResponse::WriteMessageSuccess { repr: persisted_repr, instance_id });
  assert_eq!(context.state(), PersistentActorState::PersistingEvents);
  assert!(context.should_stash_commands());

  let _ = context.handle_journal_response(&JournalResponse::WriteMessagesSuccessful { instance_id });
  assert_eq!(context.state(), PersistentActorState::ProcessingCommands);
  assert!(!context.should_stash_commands());
}

#[test]
fn should_stash_commands_until_batch_success_when_stashing_defer_is_between_persisted_events() {
  let (journal_ref, journal_store) = create_sender();
  let (snapshot_ref, _snapshot_store) = create_sender();
  let mut context = DummyContext::new("pid-1".to_string());
  context.bind_actor_refs(journal_ref, snapshot_ref).expect("bind actor refs");
  context.state = PersistentActorState::ProcessingCommands;

  context.add_to_event_batch(1_i32, false, None, Box::new(|_actor: &mut DummyActor, _repr| {}));
  context.add_deferred_handler(2_i32, true, None, Box::new(|_actor: &mut DummyActor, _repr| {}));
  context.add_to_event_batch(3_i32, false, None, Box::new(|_actor: &mut DummyActor, _repr| {}));
  context.flush_batch(ActorRefGeneric::null()).expect("flush batch");

  let persisted_reprs = {
    let journal_messages = journal_store.lock();
    let message = journal_messages[0].payload().downcast_ref::<JournalMessage<TB>>().expect("unexpected payload");
    match message {
      | JournalMessage::WriteMessages { messages, .. } => messages.clone(),
      | _ => panic!("unexpected message"),
    }
  };
  assert_eq!(persisted_reprs.len(), 2);

  let instance_id = context.instance_id();
  let _ = context
    .handle_journal_response(&JournalResponse::WriteMessageSuccess { repr: persisted_reprs[0].clone(), instance_id });
  assert_eq!(context.state(), PersistentActorState::PersistingEvents);
  assert!(context.should_stash_commands());

  let _ = context
    .handle_journal_response(&JournalResponse::WriteMessageSuccess { repr: persisted_reprs[1].clone(), instance_id });
  assert_eq!(context.state(), PersistentActorState::PersistingEvents);
  assert!(context.should_stash_commands());

  let _ = context.handle_journal_response(&JournalResponse::WriteMessagesSuccessful { instance_id });
  assert_eq!(context.state(), PersistentActorState::ProcessingCommands);
  assert!(!context.should_stash_commands());
}

#[test]
fn write_message_failure_keeps_context_in_persisting_state() {
  let (journal_ref, journal_store) = create_sender();
  let (snapshot_ref, _snapshot_store) = create_sender();
  let mut context = DummyContext::new("pid-1".to_string());
  context.bind_actor_refs(journal_ref, snapshot_ref).expect("bind actor refs");
  context.state = PersistentActorState::ProcessingCommands;

  context.add_to_event_batch(1_i32, true, None, Box::new(|_actor: &mut DummyActor, _repr| {}));
  context.flush_batch(ActorRefGeneric::null()).expect("flush batch");

  let failed_repr = {
    let journal_messages = journal_store.lock();
    let message = journal_messages[0].payload().downcast_ref::<JournalMessage<TB>>().expect("unexpected payload");
    match message {
      | JournalMessage::WriteMessages { messages, .. } => messages[0].clone(),
      | _ => panic!("unexpected message"),
    }
  };

  let instance_id = context.instance_id();
  let action = context.handle_journal_response(&JournalResponse::WriteMessageFailure {
    repr: failed_repr.clone(),
    cause: JournalError::WriteFailed("write failed".to_string()),
    instance_id,
  });
  match action {
    | JournalResponseAction::PersistFailure { cause, repr } => {
      assert_eq!(cause, JournalError::WriteFailed("write failed".to_string()));
      assert_eq!(repr.sequence_nr(), failed_repr.sequence_nr());
    },
    | _ => panic!("expected persist failure action"),
  }
  assert_eq!(context.state(), PersistentActorState::PersistingEvents);

  context.add_to_event_batch(2_i32, true, None, Box::new(|_actor: &mut DummyActor, _repr| {}));
  let result = context.flush_batch(ActorRefGeneric::null());
  assert!(result.is_err());
}

#[test]
fn write_message_failure_with_mismatched_instance_id_is_ignored() {
  let (journal_ref, journal_store) = create_sender();
  let (snapshot_ref, _snapshot_store) = create_sender();
  let mut context = DummyContext::new("pid-1".to_string());
  context.bind_actor_refs(journal_ref, snapshot_ref).expect("bind actor refs");
  context.state = PersistentActorState::ProcessingCommands;

  context.add_to_event_batch(1_i32, true, None, Box::new(|_actor: &mut DummyActor, _repr| {}));
  context.flush_batch(ActorRefGeneric::null()).expect("flush batch");

  let failed_repr = {
    let journal_messages = journal_store.lock();
    let message = journal_messages[0].payload().downcast_ref::<JournalMessage<TB>>().expect("unexpected payload");
    match message {
      | JournalMessage::WriteMessages { messages, .. } => messages[0].clone(),
      | _ => panic!("unexpected message"),
    }
  };

  let mismatched_instance_id = context.instance_id().wrapping_add(1);
  let action = context.handle_journal_response(&JournalResponse::WriteMessageFailure {
    repr:        failed_repr,
    cause:       JournalError::WriteFailed("write failed".to_string()),
    instance_id: mismatched_instance_id,
  });
  assert!(matches!(action, JournalResponseAction::None));
  assert_eq!(context.state(), PersistentActorState::PersistingEvents);
}

#[test]
fn write_message_rejected_returns_to_processing_commands() {
  let (journal_ref, journal_store) = create_sender();
  let (snapshot_ref, _snapshot_store) = create_sender();
  let mut context = DummyContext::new("pid-1".to_string());
  context.bind_actor_refs(journal_ref, snapshot_ref).expect("bind actor refs");
  context.state = PersistentActorState::ProcessingCommands;

  context.add_to_event_batch(1_i32, true, None, Box::new(|_actor: &mut DummyActor, _repr| {}));
  context.flush_batch(ActorRefGeneric::null()).expect("flush batch");

  let rejected_repr = {
    let journal_messages = journal_store.lock();
    let message = journal_messages[0].payload().downcast_ref::<JournalMessage<TB>>().expect("unexpected payload");
    match message {
      | JournalMessage::WriteMessages { messages, .. } => messages[0].clone(),
      | _ => panic!("unexpected message"),
    }
  };

  let instance_id = context.instance_id();
  let action = context.handle_journal_response(&JournalResponse::WriteMessageRejected {
    repr: rejected_repr.clone(),
    cause: JournalError::WriteFailed("write rejected".to_string()),
    instance_id,
  });
  match action {
    | JournalResponseAction::PersistRejected { cause, repr } => {
      assert_eq!(cause, JournalError::WriteFailed("write rejected".to_string()));
      assert_eq!(repr.sequence_nr(), rejected_repr.sequence_nr());
    },
    | _ => panic!("expected persist rejected action"),
  }
  assert_eq!(context.state(), PersistentActorState::ProcessingCommands);

  context.add_to_event_batch(2_i32, true, None, Box::new(|_actor: &mut DummyActor, _repr| {}));
  context.flush_batch(ActorRefGeneric::null()).expect("flush batch after rejection");
  assert_eq!(context.state(), PersistentActorState::PersistingEvents);
}

#[test]
fn write_message_rejected_with_mismatched_instance_id_is_ignored() {
  let (journal_ref, journal_store) = create_sender();
  let (snapshot_ref, _snapshot_store) = create_sender();
  let mut context = DummyContext::new("pid-1".to_string());
  context.bind_actor_refs(journal_ref, snapshot_ref).expect("bind actor refs");
  context.state = PersistentActorState::ProcessingCommands;

  context.add_to_event_batch(1_i32, true, None, Box::new(|_actor: &mut DummyActor, _repr| {}));
  context.flush_batch(ActorRefGeneric::null()).expect("flush batch");

  let rejected_repr = {
    let journal_messages = journal_store.lock();
    let message = journal_messages[0].payload().downcast_ref::<JournalMessage<TB>>().expect("unexpected payload");
    match message {
      | JournalMessage::WriteMessages { messages, .. } => messages[0].clone(),
      | _ => panic!("unexpected message"),
    }
  };

  let mismatched_instance_id = context.instance_id().wrapping_add(1);
  let action = context.handle_journal_response(&JournalResponse::WriteMessageRejected {
    repr:        rejected_repr,
    cause:       JournalError::WriteFailed("write rejected".to_string()),
    instance_id: mismatched_instance_id,
  });
  assert!(matches!(action, JournalResponseAction::None));
  assert_eq!(context.state(), PersistentActorState::PersistingEvents);
}

#[test]
fn write_messages_successful_with_mismatched_instance_id_is_ignored() {
  let (journal_ref, _journal_store) = create_sender();
  let (snapshot_ref, _snapshot_store) = create_sender();
  let mut context = DummyContext::new("pid-1".to_string());
  context.bind_actor_refs(journal_ref, snapshot_ref).expect("bind actor refs");
  context.state = PersistentActorState::ProcessingCommands;

  context.add_to_event_batch(1_i32, true, None, Box::new(|_actor: &mut DummyActor, _repr| {}));
  context.flush_batch(ActorRefGeneric::null()).expect("flush batch");

  let action = context.handle_journal_response(&JournalResponse::WriteMessagesSuccessful {
    instance_id: context.instance_id().wrapping_add(1),
  });
  assert!(matches!(action, JournalResponseAction::None));
  assert_eq!(context.state(), PersistentActorState::PersistingEvents);
}

#[test]
fn write_messages_failed_with_zero_write_count_and_mismatched_instance_id_is_ignored() {
  let (journal_ref, _journal_store) = create_sender();
  let (snapshot_ref, _snapshot_store) = create_sender();
  let mut context = DummyContext::new("pid-1".to_string());
  context.bind_actor_refs(journal_ref, snapshot_ref).expect("bind actor refs");
  context.state = PersistentActorState::ProcessingCommands;

  context.add_to_event_batch(1_i32, true, None, Box::new(|_actor: &mut DummyActor, _repr| {}));
  context.flush_batch(ActorRefGeneric::null()).expect("flush batch");

  let action = context.handle_journal_response(&JournalResponse::WriteMessagesFailed {
    cause:       JournalError::WriteFailed("batch write failed".to_string()),
    write_count: 0,
    instance_id: context.instance_id().wrapping_add(1),
  });
  assert!(matches!(action, JournalResponseAction::None));
  assert_eq!(context.state(), PersistentActorState::PersistingEvents);
}

#[test]
fn stale_write_responses_from_previous_instance_are_ignored_after_restart() {
  let stale_instance_id = {
    let (journal_ref, _journal_store) = create_sender();
    let (snapshot_ref, _snapshot_store) = create_sender();
    let mut context = DummyContext::new("pid-1".to_string());
    context.bind_actor_refs(journal_ref, snapshot_ref).expect("bind actor refs");
    context.state = PersistentActorState::ProcessingCommands;
    context.add_to_event_batch(1_i32, true, None, Box::new(|_actor: &mut DummyActor, _repr| {}));
    context.flush_batch(ActorRefGeneric::null()).expect("flush batch");
    context.instance_id()
  };

  let (journal_ref, journal_store) = create_sender();
  let (snapshot_ref, _snapshot_store) = create_sender();
  let mut success_context = DummyContext::new("pid-1".to_string());
  success_context.bind_actor_refs(journal_ref, snapshot_ref).expect("bind actor refs");
  success_context.state = PersistentActorState::ProcessingCommands;
  success_context.add_to_event_batch(
    2_i32,
    true,
    None,
    Box::new(|actor: &mut DummyActor, repr| {
      let value = repr.downcast_ref::<i32>().expect("expected i32 event");
      actor.handled_values.push(*value);
    }),
  );
  success_context.flush_batch(ActorRefGeneric::null()).expect("flush batch");

  let success_repr = {
    let journal_messages = journal_store.lock();
    let message = journal_messages[0].payload().downcast_ref::<JournalMessage<TB>>().expect("unexpected payload");
    match message {
      | JournalMessage::WriteMessages { messages, .. } => messages[0].clone(),
      | _ => panic!("unexpected message"),
    }
  };
  let current_instance_id = success_context.instance_id();
  assert_ne!(current_instance_id, stale_instance_id);

  let ignored_success = success_context.handle_journal_response(&JournalResponse::WriteMessageSuccess {
    repr:        success_repr.clone(),
    instance_id: stale_instance_id,
  });
  assert!(matches!(ignored_success, JournalResponseAction::None));

  let mut success_actor = DummyActor::default();
  let success_action = success_context.handle_journal_response(&JournalResponse::WriteMessageSuccess {
    repr:        success_repr,
    instance_id: current_instance_id,
  });
  success_action.apply::<TB>(&mut success_actor);
  assert_eq!(success_actor.handled_values, vec![2_i32]);

  let (journal_ref, journal_store) = create_sender();
  let (snapshot_ref, _snapshot_store) = create_sender();
  let mut failure_context = DummyContext::new("pid-1".to_string());
  failure_context.bind_actor_refs(journal_ref, snapshot_ref).expect("bind actor refs");
  failure_context.state = PersistentActorState::ProcessingCommands;
  failure_context.add_to_event_batch(3_i32, true, None, Box::new(|_actor: &mut DummyActor, _repr| {}));
  failure_context.flush_batch(ActorRefGeneric::null()).expect("flush batch");

  let failure_repr = {
    let journal_messages = journal_store.lock();
    let message = journal_messages[0].payload().downcast_ref::<JournalMessage<TB>>().expect("unexpected payload");
    match message {
      | JournalMessage::WriteMessages { messages, .. } => messages[0].clone(),
      | _ => panic!("unexpected message"),
    }
  };
  assert_ne!(failure_context.instance_id(), stale_instance_id);
  let ignored_failure = failure_context.handle_journal_response(&JournalResponse::WriteMessageFailure {
    repr:        failure_repr,
    cause:       JournalError::WriteFailed("stale failure".to_string()),
    instance_id: stale_instance_id,
  });
  assert!(matches!(ignored_failure, JournalResponseAction::None));
  assert_eq!(failure_context.state(), PersistentActorState::PersistingEvents);

  let (journal_ref, journal_store) = create_sender();
  let (snapshot_ref, _snapshot_store) = create_sender();
  let mut rejected_context = DummyContext::new("pid-1".to_string());
  rejected_context.bind_actor_refs(journal_ref, snapshot_ref).expect("bind actor refs");
  rejected_context.state = PersistentActorState::ProcessingCommands;
  rejected_context.add_to_event_batch(4_i32, true, None, Box::new(|_actor: &mut DummyActor, _repr| {}));
  rejected_context.flush_batch(ActorRefGeneric::null()).expect("flush batch");

  let rejected_repr = {
    let journal_messages = journal_store.lock();
    let message = journal_messages[0].payload().downcast_ref::<JournalMessage<TB>>().expect("unexpected payload");
    match message {
      | JournalMessage::WriteMessages { messages, .. } => messages[0].clone(),
      | _ => panic!("unexpected message"),
    }
  };
  assert_ne!(rejected_context.instance_id(), stale_instance_id);
  let ignored_rejected = rejected_context.handle_journal_response(&JournalResponse::WriteMessageRejected {
    repr:        rejected_repr,
    cause:       JournalError::WriteFailed("stale rejected".to_string()),
    instance_id: stale_instance_id,
  });
  assert!(matches!(ignored_rejected, JournalResponseAction::None));
  assert_eq!(rejected_context.state(), PersistentActorState::PersistingEvents);

  let (journal_ref, _journal_store) = create_sender();
  let (snapshot_ref, _snapshot_store) = create_sender();
  let mut batch_success_context = DummyContext::new("pid-1".to_string());
  batch_success_context.bind_actor_refs(journal_ref, snapshot_ref).expect("bind actor refs");
  batch_success_context.state = PersistentActorState::PersistingEvents;
  batch_success_context.add_deferred_handler(
    5_i32,
    false,
    None,
    Box::new(|actor: &mut DummyActor, repr| {
      let value = repr.downcast_ref::<i32>().expect("expected i32 event");
      actor.handled_values.push(*value);
    }),
  );
  assert_ne!(batch_success_context.instance_id(), stale_instance_id);
  let ignored_batch_success = batch_success_context
    .handle_journal_response(&JournalResponse::WriteMessagesSuccessful { instance_id: stale_instance_id });
  assert!(matches!(ignored_batch_success, JournalResponseAction::None));
  assert_eq!(batch_success_context.state(), PersistentActorState::PersistingEvents);

  let mut batch_success_actor = DummyActor::default();
  let apply_batch_success = batch_success_context.handle_journal_response(&JournalResponse::WriteMessagesSuccessful {
    instance_id: batch_success_context.instance_id(),
  });
  apply_batch_success.apply::<TB>(&mut batch_success_actor);
  assert_eq!(batch_success_actor.handled_values, vec![5_i32]);
  assert_eq!(batch_success_context.state(), PersistentActorState::ProcessingCommands);

  let (journal_ref, _journal_store) = create_sender();
  let (snapshot_ref, _snapshot_store) = create_sender();
  let mut batch_failure_context = DummyContext::new("pid-1".to_string());
  batch_failure_context.bind_actor_refs(journal_ref, snapshot_ref).expect("bind actor refs");
  batch_failure_context.state = PersistentActorState::ProcessingCommands;
  batch_failure_context.add_to_event_batch(6_i32, true, None, Box::new(|_actor: &mut DummyActor, _repr| {}));
  batch_failure_context.flush_batch(ActorRefGeneric::null()).expect("flush batch");
  assert_ne!(batch_failure_context.instance_id(), stale_instance_id);
  let ignored_batch_failure = batch_failure_context.handle_journal_response(&JournalResponse::WriteMessagesFailed {
    cause:       JournalError::WriteFailed("stale batch failed".to_string()),
    write_count: 0,
    instance_id: stale_instance_id,
  });
  assert!(matches!(ignored_batch_failure, JournalResponseAction::None));
  assert_eq!(batch_failure_context.state(), PersistentActorState::PersistingEvents);
}

#[test]
fn write_message_rejected_keeps_remaining_invocations() {
  let (journal_ref, journal_store) = create_sender();
  let (snapshot_ref, _snapshot_store) = create_sender();
  let mut context = DummyContext::new("pid-1".to_string());
  context.bind_actor_refs(journal_ref, snapshot_ref).expect("bind actor refs");
  context.state = PersistentActorState::ProcessingCommands;

  context.add_to_event_batch(
    1_i32,
    false,
    None,
    Box::new(|actor: &mut DummyActor, repr| {
      let value = repr.downcast_ref::<i32>().expect("expected i32 event");
      actor.handled_values.push(*value);
    }),
  );
  context.add_deferred_handler(
    2_i32,
    false,
    None,
    Box::new(|actor: &mut DummyActor, repr| {
      let value = repr.downcast_ref::<i32>().expect("expected i32 event");
      actor.handled_values.push(*value);
    }),
  );
  context.add_to_event_batch(
    3_i32,
    false,
    None,
    Box::new(|actor: &mut DummyActor, repr| {
      let value = repr.downcast_ref::<i32>().expect("expected i32 event");
      actor.handled_values.push(*value);
    }),
  );
  context.flush_batch(ActorRefGeneric::null()).expect("flush batch");

  let persisted_reprs = {
    let journal_messages = journal_store.lock();
    let message = journal_messages[0].payload().downcast_ref::<JournalMessage<TB>>().expect("unexpected payload");
    match message {
      | JournalMessage::WriteMessages { messages, .. } => messages.clone(),
      | _ => panic!("unexpected message"),
    }
  };
  assert_eq!(persisted_reprs.len(), 2);

  let instance_id = context.instance_id();
  let rejected_action = context.handle_journal_response(&JournalResponse::WriteMessageRejected {
    repr: persisted_reprs[0].clone(),
    cause: JournalError::WriteFailed("write rejected".to_string()),
    instance_id,
  });
  match rejected_action {
    | JournalResponseAction::PersistRejected { cause, repr } => {
      assert_eq!(cause, JournalError::WriteFailed("write rejected".to_string()));
      assert_eq!(repr.sequence_nr(), persisted_reprs[0].sequence_nr());
    },
    | _ => panic!("expected persist rejected action"),
  }
  assert_eq!(context.state(), PersistentActorState::PersistingEvents);

  let mut actor = DummyActor::default();
  let success_action = context
    .handle_journal_response(&JournalResponse::WriteMessageSuccess { repr: persisted_reprs[1].clone(), instance_id });
  success_action.apply::<TB>(&mut actor);
  assert_eq!(actor.handled_values, vec![2_i32, 3_i32]);

  let completion_action = context.handle_journal_response(&JournalResponse::WriteMessagesSuccessful { instance_id });
  completion_action.apply::<TB>(&mut actor);
  assert_eq!(context.state(), PersistentActorState::ProcessingCommands);
  assert_eq!(actor.handled_values, vec![2_i32, 3_i32]);
}

#[test]
fn write_message_rejected_for_later_persist_keeps_deferred_invocation() {
  let (journal_ref, journal_store) = create_sender();
  let (snapshot_ref, _snapshot_store) = create_sender();
  let mut context = DummyContext::new("pid-1".to_string());
  context.bind_actor_refs(journal_ref, snapshot_ref).expect("bind actor refs");
  context.state = PersistentActorState::ProcessingCommands;

  context.add_to_event_batch(
    1_i32,
    false,
    None,
    Box::new(|actor: &mut DummyActor, repr| {
      let value = repr.downcast_ref::<i32>().expect("expected i32 event");
      actor.handled_values.push(*value);
    }),
  );
  context.add_deferred_handler(
    2_i32,
    false,
    None,
    Box::new(|actor: &mut DummyActor, repr| {
      let value = repr.downcast_ref::<i32>().expect("expected i32 event");
      actor.handled_values.push(*value);
    }),
  );
  context.add_to_event_batch(
    3_i32,
    false,
    None,
    Box::new(|actor: &mut DummyActor, repr| {
      let value = repr.downcast_ref::<i32>().expect("expected i32 event");
      actor.handled_values.push(*value);
    }),
  );
  context.flush_batch(ActorRefGeneric::null()).expect("flush batch");

  let persisted_reprs = {
    let journal_messages = journal_store.lock();
    let message = journal_messages[0].payload().downcast_ref::<JournalMessage<TB>>().expect("unexpected payload");
    match message {
      | JournalMessage::WriteMessages { messages, .. } => messages.clone(),
      | _ => panic!("unexpected message"),
    }
  };
  assert_eq!(persisted_reprs.len(), 2);

  let instance_id = context.instance_id();
  let mut actor = DummyActor::default();
  let first_action = context
    .handle_journal_response(&JournalResponse::WriteMessageSuccess { repr: persisted_reprs[0].clone(), instance_id });
  first_action.apply::<TB>(&mut actor);
  assert_eq!(actor.handled_values, vec![1_i32]);

  let rejected_action = context.handle_journal_response(&JournalResponse::WriteMessageRejected {
    repr: persisted_reprs[1].clone(),
    cause: JournalError::WriteFailed("write rejected".to_string()),
    instance_id,
  });
  match rejected_action {
    | JournalResponseAction::PersistRejected { cause, repr } => {
      assert_eq!(cause, JournalError::WriteFailed("write rejected".to_string()));
      assert_eq!(repr.sequence_nr(), persisted_reprs[1].sequence_nr());
    },
    | _ => panic!("expected persist rejected action"),
  }
  assert_eq!(context.state(), PersistentActorState::PersistingEvents);

  let completion_action = context.handle_journal_response(&JournalResponse::WriteMessagesSuccessful { instance_id });
  completion_action.apply::<TB>(&mut actor);
  assert_eq!(context.state(), PersistentActorState::ProcessingCommands);
  assert_eq!(actor.handled_values, vec![1_i32, 2_i32]);
}

#[test]
fn write_messages_failed_with_positive_write_count_keeps_persisting_state() {
  let (journal_ref, _journal_store) = create_sender();
  let (snapshot_ref, _snapshot_store) = create_sender();
  let mut context = DummyContext::new("pid-1".to_string());
  context.bind_actor_refs(journal_ref, snapshot_ref).expect("bind actor refs");
  context.state = PersistentActorState::ProcessingCommands;

  context.add_to_event_batch(1_i32, true, None, Box::new(|_actor: &mut DummyActor, _repr| {}));
  context.flush_batch(ActorRefGeneric::null()).expect("flush batch");
  assert_eq!(context.state(), PersistentActorState::PersistingEvents);

  let action = context.handle_journal_response(&JournalResponse::WriteMessagesFailed {
    cause:       JournalError::WriteFailed("batch write failed".to_string()),
    write_count: 1,
    instance_id: context.instance_id(),
  });
  assert!(matches!(action, JournalResponseAction::None));
  assert_eq!(context.state(), PersistentActorState::PersistingEvents);

  context.add_to_event_batch(2_i32, true, None, Box::new(|_actor: &mut DummyActor, _repr| {}));
  let result = context.flush_batch(ActorRefGeneric::null());
  assert!(result.is_err());
}

#[test]
fn write_messages_failed_with_zero_write_count_returns_to_processing_commands() {
  let (journal_ref, _journal_store) = create_sender();
  let (snapshot_ref, _snapshot_store) = create_sender();
  let mut context = DummyContext::new("pid-1".to_string());
  context.bind_actor_refs(journal_ref, snapshot_ref).expect("bind actor refs");
  context.state = PersistentActorState::ProcessingCommands;

  context.add_to_event_batch(1_i32, true, None, Box::new(|_actor: &mut DummyActor, _repr| {}));
  context.flush_batch(ActorRefGeneric::null()).expect("flush batch");
  assert_eq!(context.state(), PersistentActorState::PersistingEvents);

  let action = context.handle_journal_response(&JournalResponse::WriteMessagesFailed {
    cause:       JournalError::WriteFailed("batch write failed".to_string()),
    write_count: 0,
    instance_id: context.instance_id(),
  });
  assert!(matches!(action, JournalResponseAction::None));
  assert_eq!(context.state(), PersistentActorState::ProcessingCommands);

  context.add_to_event_batch(2_i32, true, None, Box::new(|_actor: &mut DummyActor, _repr| {}));
  context.flush_batch(ActorRefGeneric::null()).expect("flush batch after write messages failed");
  assert_eq!(context.state(), PersistentActorState::PersistingEvents);
}

#[test]
fn write_messages_successful_is_ignored_during_recovery() {
  let mut context = DummyContext::new("pid-1".to_string());
  context.state = PersistentActorState::Recovering;

  let action =
    context.handle_journal_response(&JournalResponse::WriteMessagesSuccessful { instance_id: context.instance_id() });

  assert!(matches!(action, JournalResponseAction::None));
  assert_eq!(context.state(), PersistentActorState::Recovering);
}

#[test]
fn write_messages_failed_with_zero_write_count_is_ignored_during_recovery() {
  let mut context = DummyContext::new("pid-1".to_string());
  context.state = PersistentActorState::RecoveryStarted;

  let action = context.handle_journal_response(&JournalResponse::WriteMessagesFailed {
    cause:       JournalError::WriteFailed("batch write failed".to_string()),
    write_count: 0,
    instance_id: context.instance_id(),
  });

  assert!(matches!(action, JournalResponseAction::None));
  assert_eq!(context.state(), PersistentActorState::RecoveryStarted);
}
