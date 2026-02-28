use alloc::vec;

use fraktor_actor_rs::core::{
  actor::{
    Actor, ActorCellGeneric, ActorContextGeneric, Pid,
    actor_ref::{ActorRefGeneric, ActorRefSender, SendOutcome},
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
  in_memory_journal::InMemoryJournal, journal_actor::JournalActor, journal_actor_config::JournalActorConfig,
  journal_error::JournalError, journal_message::JournalMessage, journal_response::JournalResponse,
  persistent_repr::PersistentRepr,
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

fn new_test_system() -> ActorSystemGeneric<TB> {
  let state = SystemStateGeneric::new();
  let state = SystemStateSharedGeneric::new(state);
  state.mark_root_started();
  register_actor_cell(&state, Pid::new(1, 1));
  ActorSystemGeneric::from_state(state)
}

struct DummyActor;

impl Actor<TB> for DummyActor {
  fn receive(
    &mut self,
    _ctx: &mut ActorContextGeneric<'_, TB>,
    _message: AnyMessageViewGeneric<'_, TB>,
  ) -> Result<(), ActorError> {
    Ok(())
  }
}

fn register_actor_cell(state: &SystemStateSharedGeneric<TB>, pid: Pid) {
  let props = PropsGeneric::from_fn(|| DummyActor);
  let cell = ActorCellGeneric::create(state.clone(), pid, None, "test".into(), &props).expect("cell create failed");
  state.register_cell(cell);
}

struct PendingJournal;

impl crate::core::journal::Journal for PendingJournal {
  type DeleteFuture<'a>
    = core::future::Pending<Result<(), JournalError>>
  where
    Self: 'a;
  type HighestSeqNrFuture<'a>
    = core::future::Pending<Result<u64, JournalError>>
  where
    Self: 'a;
  type ReplayFuture<'a>
    = core::future::Pending<Result<alloc::vec::Vec<PersistentRepr>, JournalError>>
  where
    Self: 'a;
  type WriteFuture<'a>
    = core::future::Pending<Result<(), JournalError>>
  where
    Self: 'a;

  fn write_messages<'a>(&'a mut self, _messages: &'a [PersistentRepr]) -> Self::WriteFuture<'a> {
    core::future::pending()
  }

  fn replay_messages<'a>(
    &'a self,
    _persistence_id: &'a str,
    _from_sequence_nr: u64,
    _to_sequence_nr: u64,
    _max: u64,
  ) -> Self::ReplayFuture<'a> {
    core::future::pending()
  }

  fn delete_messages_to<'a>(&'a mut self, _persistence_id: &'a str, _to_sequence_nr: u64) -> Self::DeleteFuture<'a> {
    core::future::pending()
  }

  fn highest_sequence_nr<'a>(&'a self, _persistence_id: &'a str) -> Self::HighestSeqNrFuture<'a> {
    core::future::pending()
  }
}

struct RetryJournal {
  failures_left: u32,
}

impl RetryJournal {
  fn new(failures_left: u32) -> Self {
    Self { failures_left }
  }
}

impl crate::core::journal::Journal for RetryJournal {
  type DeleteFuture<'a>
    = core::future::Ready<Result<(), JournalError>>
  where
    Self: 'a;
  type HighestSeqNrFuture<'a>
    = core::future::Ready<Result<u64, JournalError>>
  where
    Self: 'a;
  type ReplayFuture<'a>
    = core::future::Ready<Result<alloc::vec::Vec<PersistentRepr>, JournalError>>
  where
    Self: 'a;
  type WriteFuture<'a>
    = core::future::Ready<Result<(), JournalError>>
  where
    Self: 'a;

  fn write_messages<'a>(&'a mut self, _messages: &'a [PersistentRepr]) -> Self::WriteFuture<'a> {
    if self.failures_left > 0 {
      self.failures_left -= 1;
      core::future::ready(Err(JournalError::WriteFailed("boom".into())))
    } else {
      core::future::ready(Ok(()))
    }
  }

  fn replay_messages<'a>(
    &'a self,
    _persistence_id: &'a str,
    _from_sequence_nr: u64,
    _to_sequence_nr: u64,
    _max: u64,
  ) -> Self::ReplayFuture<'a> {
    core::future::ready(Ok(alloc::vec::Vec::new()))
  }

  fn delete_messages_to<'a>(&'a mut self, _persistence_id: &'a str, _to_sequence_nr: u64) -> Self::DeleteFuture<'a> {
    core::future::ready(Ok(()))
  }

  fn highest_sequence_nr<'a>(&'a self, _persistence_id: &'a str) -> Self::HighestSeqNrFuture<'a> {
    core::future::ready(Ok(0))
  }
}

#[test]
fn journal_actor_write_messages_sends_responses() {
  let system = new_test_system();
  let pid = Pid::new(1, 1);
  let mut ctx = ActorContextGeneric::new(&system, pid);
  let mut actor = JournalActor::<InMemoryJournal, TB>::new(InMemoryJournal::new());
  let (sender, store) = create_sender();

  let payload1 = ArcShared::new(1_i32);
  let payload2 = ArcShared::new(2_i32);
  let repr1 = PersistentRepr::new("pid-1", 1, payload1);
  let repr2 = PersistentRepr::new("pid-1", 2, payload2);
  let message = JournalMessage::WriteMessages {
    persistence_id: "pid-1".into(),
    to_sequence_nr: 2,
    messages: vec![repr1, repr2],
    sender,
    instance_id: 9,
  };

  let any_message = AnyMessageGeneric::new(message);
  actor.receive(&mut ctx, any_message.as_view()).expect("receive failed");

  let responses = store.lock();
  assert_eq!(responses.len(), 3);
  let mut success_count = 0;
  let mut batch_success = 0;
  let mut batch_success_instance_id = None;
  for response in responses.iter() {
    let response = response.payload().downcast_ref::<JournalResponse>().expect("unexpected payload");
    match response {
      | JournalResponse::WriteMessageSuccess { .. } => success_count += 1,
      | JournalResponse::WriteMessagesSuccessful { instance_id } => {
        batch_success += 1;
        batch_success_instance_id = Some(*instance_id);
      },
      | _ => {},
    }
  }
  assert_eq!(success_count, 2);
  assert_eq!(batch_success, 1);
  assert_eq!(batch_success_instance_id, Some(9));
}

#[test]
fn journal_actor_pending_does_not_emit_failure() {
  let system = new_test_system();
  let pid = Pid::new(1, 1);
  let mut ctx = ActorContextGeneric::new(&system, pid);
  let config = JournalActorConfig::new(0);
  let mut actor = JournalActor::<PendingJournal, TB>::new_with_config(PendingJournal, config);
  let (sender, store) = create_sender();

  let payload = ArcShared::new(1_i32);
  let repr = PersistentRepr::new("pid-1", 1, payload);
  let message = JournalMessage::WriteMessages {
    persistence_id: "pid-1".into(),
    to_sequence_nr: 1,
    messages: vec![repr],
    sender,
    instance_id: 1,
  };

  let any_message = AnyMessageGeneric::new(message);
  actor.receive(&mut ctx, any_message.as_view()).expect("receive failed");

  let responses = store.lock();
  assert_eq!(responses.len(), 0);
}

#[test]
fn journal_actor_retry_max_exceeded_on_errors() {
  let system = new_test_system();
  let pid = Pid::new(1, 1);
  let mut ctx = ActorContextGeneric::new(&system, pid);
  let config = JournalActorConfig::new(1);
  let mut actor = JournalActor::<RetryJournal, TB>::new_with_config(RetryJournal::new(2), config);
  let (sender, store) = create_sender();

  let payload = ArcShared::new(1_i32);
  let repr = PersistentRepr::new("pid-1", 1, payload);
  let message = JournalMessage::WriteMessages {
    persistence_id: "pid-1".into(),
    to_sequence_nr: 1,
    messages: vec![repr],
    sender,
    instance_id: 1,
  };

  let any_message = AnyMessageGeneric::new(message);
  actor.receive(&mut ctx, any_message.as_view()).expect("receive failed");

  assert!(store.lock().is_empty());

  let poll = AnyMessageGeneric::new(super::JournalPoll);
  actor.receive(&mut ctx, poll.as_view()).expect("receive failed");

  let responses = store.lock();
  assert_eq!(responses.len(), 2);
  let mut failures = 0;
  let mut batch_failures = 0;
  for response in responses.iter() {
    let response = response.payload().downcast_ref::<JournalResponse>().expect("unexpected payload");
    match response {
      | JournalResponse::WriteMessageFailure { cause, .. } => {
        assert_eq!(cause, &JournalError::WriteFailed("boom".into()));
        failures += 1;
      },
      | JournalResponse::WriteMessagesFailed { cause, instance_id, .. } => {
        assert_eq!(cause, &JournalError::WriteFailed("boom".into()));
        assert_eq!(*instance_id, 1);
        batch_failures += 1;
      },
      | _ => {},
    }
  }
  assert_eq!(failures, 1);
  assert_eq!(batch_failures, 1);
}

#[test]
fn journal_actor_replay_filters_deleted_messages() {
  let system = new_test_system();
  let pid = Pid::new(1, 1);
  let mut ctx = ActorContextGeneric::new(&system, pid);
  let mut actor = JournalActor::<InMemoryJournal, TB>::new(InMemoryJournal::new());
  let (sender, store) = create_sender();

  let repr1 = PersistentRepr::new("pid-1", 1, ArcShared::new(1_i32));
  let repr2 = PersistentRepr::new("pid-1", 2, ArcShared::new(2_i32)).with_deleted(true);
  let repr3 = PersistentRepr::new("pid-1", 3, ArcShared::new(3_i32));
  let write = JournalMessage::WriteMessages {
    persistence_id: "pid-1".into(),
    to_sequence_nr: 3,
    messages:       vec![repr1, repr2, repr3],
    sender:         sender.clone(),
    instance_id:    10,
  };
  let write_message = AnyMessageGeneric::new(write);
  actor.receive(&mut ctx, write_message.as_view()).expect("write receive failed");
  store.lock().clear();

  let replay = JournalMessage::ReplayMessages {
    persistence_id: "pid-1".into(),
    from_sequence_nr: 1,
    to_sequence_nr: 3,
    max: 10,
    sender,
  };
  let replay_message = AnyMessageGeneric::new(replay);
  actor.receive(&mut ctx, replay_message.as_view()).expect("replay receive failed");

  let responses = store.lock();
  let mut replayed = Vec::new();
  let mut recovery_highest = None;
  for response in responses.iter() {
    let response = response.payload().downcast_ref::<JournalResponse>().expect("unexpected payload");
    match response {
      | JournalResponse::ReplayedMessage { persistent_repr } => replayed.push(persistent_repr.sequence_nr()),
      | JournalResponse::RecoverySuccess { highest_sequence_nr } => recovery_highest = Some(*highest_sequence_nr),
      | _ => {},
    }
  }

  assert_eq!(replayed, vec![1, 3]);
  assert_eq!(recovery_highest, Some(3));
}
