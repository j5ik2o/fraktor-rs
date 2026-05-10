use alloc::{vec, vec::Vec};
use core::future::{Pending, Ready, pending, ready};

use fraktor_actor_adaptor_std_rs::system::create_noop_actor_system;
use fraktor_actor_core_kernel_rs::{
  actor::{
    Actor, ActorCell, ActorContext, Pid,
    actor_ref::{ActorRef, ActorRefSender, ActorRefSenderShared, SendOutcome},
    error::{ActorError, SendError},
    messaging::{AnyMessage, AnyMessageView},
    props::Props,
  },
  system::{ActorSystem, state::SystemStateShared},
};
use fraktor_utils_core_rs::sync::{ArcShared, SharedLock, SpinSyncMutex};

use super::JournalPoll;
use crate::{
  in_memory_journal::InMemoryJournal, journal_actor::JournalActor, journal_actor_config::JournalActorConfig,
  journal_error::JournalError, journal_message::JournalMessage, journal_response::JournalResponse,
  persistent_repr::PersistentRepr,
};

type MessageStore = ArcShared<SpinSyncMutex<Vec<AnyMessage>>>;

struct TestSender {
  messages: MessageStore,
}

impl ActorRefSender for TestSender {
  fn send(&mut self, message: AnyMessage) -> Result<SendOutcome, SendError> {
    self.messages.lock().push(message);
    Ok(SendOutcome::Delivered)
  }
}

fn actor_ref_with_sender(pid: Pid, sender: impl ActorRefSender + 'static) -> ActorRef {
  let sender = ActorRefSenderShared::from_shared_lock(SharedLock::new_with_driver::<
    SpinSyncMutex<Box<dyn ActorRefSender>>,
  >(Box::new(sender)));
  ActorRef::new(pid, sender)
}

fn create_sender() -> (ActorRef, MessageStore) {
  let messages = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let sender = actor_ref_with_sender(Pid::new(1, 1), TestSender { messages: messages.clone() });
  (sender, messages)
}

fn test_actor_pid() -> Pid {
  Pid::new(10_000, 1)
}

fn new_test_system() -> ActorSystem {
  let system = create_noop_actor_system();
  register_actor_cell(&system.state(), test_actor_pid());
  system
}

struct DummyActor;

impl Actor for DummyActor {
  fn receive(&mut self, _ctx: &mut ActorContext<'_>, _message: AnyMessageView<'_>) -> Result<(), ActorError> {
    Ok(())
  }
}

fn register_actor_cell(state: &SystemStateShared, pid: Pid) {
  let props = Props::from_fn(|| DummyActor);
  let cell = ActorCell::create(state.clone(), pid, None, "test".into(), &props).expect("cell create failed");
  state.register_cell(cell);
}

struct PendingJournal;

impl crate::journal::Journal for PendingJournal {
  type DeleteFuture<'a>
    = Pending<Result<(), JournalError>>
  where
    Self: 'a;
  type HighestSeqNrFuture<'a>
    = Pending<Result<u64, JournalError>>
  where
    Self: 'a;
  type ReplayFuture<'a>
    = Pending<Result<Vec<PersistentRepr>, JournalError>>
  where
    Self: 'a;
  type WriteFuture<'a>
    = Pending<Result<(), JournalError>>
  where
    Self: 'a;

  fn write_messages<'a>(&'a mut self, _messages: &'a [PersistentRepr]) -> Self::WriteFuture<'a> {
    pending()
  }

  fn replay_messages<'a>(
    &'a self,
    _persistence_id: &'a str,
    _from_sequence_nr: u64,
    _to_sequence_nr: u64,
    _max: u64,
  ) -> Self::ReplayFuture<'a> {
    pending()
  }

  fn delete_messages_to<'a>(&'a mut self, _persistence_id: &'a str, _to_sequence_nr: u64) -> Self::DeleteFuture<'a> {
    pending()
  }

  fn highest_sequence_nr<'a>(&'a self, _persistence_id: &'a str) -> Self::HighestSeqNrFuture<'a> {
    pending()
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

impl crate::journal::Journal for RetryJournal {
  type DeleteFuture<'a>
    = Ready<Result<(), JournalError>>
  where
    Self: 'a;
  type HighestSeqNrFuture<'a>
    = Ready<Result<u64, JournalError>>
  where
    Self: 'a;
  type ReplayFuture<'a>
    = Ready<Result<Vec<PersistentRepr>, JournalError>>
  where
    Self: 'a;
  type WriteFuture<'a>
    = Ready<Result<(), JournalError>>
  where
    Self: 'a;

  fn write_messages<'a>(&'a mut self, _messages: &'a [PersistentRepr]) -> Self::WriteFuture<'a> {
    if self.failures_left > 0 {
      self.failures_left -= 1;
      ready(Err(JournalError::WriteFailed("boom".into())))
    } else {
      ready(Ok(()))
    }
  }

  fn replay_messages<'a>(
    &'a self,
    _persistence_id: &'a str,
    _from_sequence_nr: u64,
    _to_sequence_nr: u64,
    _max: u64,
  ) -> Self::ReplayFuture<'a> {
    ready(Ok(Vec::new()))
  }

  fn delete_messages_to<'a>(&'a mut self, _persistence_id: &'a str, _to_sequence_nr: u64) -> Self::DeleteFuture<'a> {
    ready(Ok(()))
  }

  fn highest_sequence_nr<'a>(&'a self, _persistence_id: &'a str) -> Self::HighestSeqNrFuture<'a> {
    ready(Ok(0))
  }
}

#[test]
fn journal_actor_write_messages_sends_responses() {
  let system = new_test_system();
  let pid = test_actor_pid();
  let mut ctx = ActorContext::new(&system, pid);
  let mut actor = JournalActor::<InMemoryJournal>::new(InMemoryJournal::new());
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

  let any_message = AnyMessage::new(message);
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
  let pid = test_actor_pid();
  let mut ctx = ActorContext::new(&system, pid);
  let config = JournalActorConfig::new(0);
  let mut actor = JournalActor::<PendingJournal>::new_with_config(PendingJournal, config);
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

  let any_message = AnyMessage::new(message);
  actor.receive(&mut ctx, any_message.as_view()).expect("receive failed");

  let responses = store.lock();
  assert_eq!(responses.len(), 0);
}

#[test]
fn journal_actor_retry_max_exceeded_on_errors() {
  let system = new_test_system();
  let pid = test_actor_pid();
  let mut ctx = ActorContext::new(&system, pid);
  let config = JournalActorConfig::new(1);
  let mut actor = JournalActor::<RetryJournal>::new_with_config(RetryJournal::new(2), config);
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

  let any_message = AnyMessage::new(message);
  actor.receive(&mut ctx, any_message.as_view()).expect("receive failed");

  assert!(store.lock().is_empty());

  let poll = AnyMessage::new(JournalPoll);
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
  let pid = test_actor_pid();
  let mut ctx = ActorContext::new(&system, pid);
  let mut actor = JournalActor::<InMemoryJournal>::new(InMemoryJournal::new());
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
  let write_message = AnyMessage::new(write);
  actor.receive(&mut ctx, write_message.as_view()).expect("write receive failed");
  store.lock().clear();

  let replay = JournalMessage::ReplayMessages {
    persistence_id: "pid-1".into(),
    from_sequence_nr: 1,
    to_sequence_nr: 3,
    max: 10,
    sender,
  };
  let replay_message = AnyMessage::new(replay);
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
