use alloc::{vec, vec::Vec};
use core::{
  future::{Pending, Ready, pending, ready},
  sync::atomic::{AtomicU32, Ordering},
};

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
  PluginMessageHandling,
  journal::{
    InMemoryJournal, JournalActor, JournalActorConfig, JournalError, JournalMessage, JournalPluginMessageHandler,
    JournalResponse,
  },
  persistent::{AtomicWrite, PersistentRepr},
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

fn atomic_write(payload: Vec<PersistentRepr>) -> AtomicWrite {
  AtomicWrite::new(payload).expect("atomic write")
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

  fn write_messages<'a>(&'a mut self, _messages: &'a [AtomicWrite]) -> Self::WriteFuture<'a> {
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

  fn write_messages<'a>(&'a mut self, _messages: &'a [AtomicWrite]) -> Self::WriteFuture<'a> {
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

struct ScriptedJournal {
  replay_failures_left:  AtomicU32,
  delete_failures_left:  AtomicU32,
  highest_failures_left: AtomicU32,
  highest_sequence_nr:   u64,
}

impl ScriptedJournal {
  fn new(replay_failures_left: u32, delete_failures_left: u32, highest_failures_left: u32) -> Self {
    Self {
      replay_failures_left:  AtomicU32::new(replay_failures_left),
      delete_failures_left:  AtomicU32::new(delete_failures_left),
      highest_failures_left: AtomicU32::new(highest_failures_left),
      highest_sequence_nr:   42,
    }
  }

  fn consume_failure(counter: &AtomicU32) -> bool {
    counter.fetch_update(Ordering::AcqRel, Ordering::Acquire, |value| value.checked_sub(1)).is_ok()
  }
}

impl crate::journal::Journal for ScriptedJournal {
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

  fn write_messages<'a>(&'a mut self, _messages: &'a [AtomicWrite]) -> Self::WriteFuture<'a> {
    ready(Ok(()))
  }

  fn replay_messages<'a>(
    &'a self,
    _persistence_id: &'a str,
    _from_sequence_nr: u64,
    _to_sequence_nr: u64,
    _max: u64,
  ) -> Self::ReplayFuture<'a> {
    if Self::consume_failure(&self.replay_failures_left) {
      ready(Err(JournalError::ReadFailed("replay failed".into())))
    } else {
      ready(Ok(Vec::new()))
    }
  }

  fn delete_messages_to<'a>(&'a mut self, _persistence_id: &'a str, _to_sequence_nr: u64) -> Self::DeleteFuture<'a> {
    if Self::consume_failure(&self.delete_failures_left) {
      ready(Err(JournalError::DeleteFailed("delete failed".into())))
    } else {
      ready(Ok(()))
    }
  }

  fn highest_sequence_nr<'a>(&'a self, _persistence_id: &'a str) -> Self::HighestSeqNrFuture<'a> {
    if Self::consume_failure(&self.highest_failures_left) {
      ready(Err(JournalError::ReadFailed("highest failed".into())))
    } else {
      ready(Ok(self.highest_sequence_nr))
    }
  }
}

struct JournalPluginCommand {
  marker: u32,
}

struct RecordingJournalPluginHandler {
  markers: ArcShared<SpinSyncMutex<Vec<u32>>>,
}

impl RecordingJournalPluginHandler {
  fn new(markers: ArcShared<SpinSyncMutex<Vec<u32>>>) -> Self {
    Self { markers }
  }
}

impl JournalPluginMessageHandler for RecordingJournalPluginHandler {
  fn handle_journal_plugin_message(
    &mut self,
    _ctx: &mut ActorContext<'_>,
    message: AnyMessageView<'_>,
  ) -> Result<PluginMessageHandling, ActorError> {
    if let Some(command) = message.downcast_ref::<JournalPluginCommand>() {
      self.markers.lock().push(command.marker);
      return Ok(PluginMessageHandling::Handled);
    }
    Ok(PluginMessageHandling::Unhandled)
  }
}

#[test]
fn scripted_journal_success_paths_are_exercised() {
  let system = new_test_system();
  let pid = test_actor_pid();
  let mut ctx = ActorContext::new(&system, pid);
  let mut actor =
    JournalActor::<ScriptedJournal>::new_with_config(ScriptedJournal::new(0, 0, 0), JournalActorConfig::new(0));
  let (sender, store) = create_sender();

  let repr = PersistentRepr::new("pid-1", 1, ArcShared::new(1_i32));
  let write = JournalMessage::WriteMessages {
    persistence_id: "pid-1".into(),
    to_sequence_nr: 1,
    messages:       vec![atomic_write(vec![repr])],
    sender:         sender.clone(),
    instance_id:    7,
  };
  let any_message = AnyMessage::new(write);
  actor.receive(&mut ctx, any_message.as_view()).expect("write receive failed");

  let replay = JournalMessage::ReplayMessages {
    persistence_id:   "pid-1".into(),
    from_sequence_nr: 1,
    to_sequence_nr:   1,
    max:              10,
    sender:           sender.clone(),
  };
  let any_message = AnyMessage::new(replay);
  actor.receive(&mut ctx, any_message.as_view()).expect("replay receive failed");

  let delete = JournalMessage::DeleteMessagesTo {
    persistence_id: "pid-1".into(),
    to_sequence_nr: 1,
    sender:         sender.clone(),
  };
  let any_message = AnyMessage::new(delete);
  actor.receive(&mut ctx, any_message.as_view()).expect("delete receive failed");

  let highest = JournalMessage::GetHighestSequenceNr { persistence_id: "pid-1".into(), from_sequence_nr: 0, sender };
  let any_message = AnyMessage::new(highest);
  actor.receive(&mut ctx, any_message.as_view()).expect("highest receive failed");

  let responses = store.lock();
  assert_eq!(responses.len(), 5);
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
    messages: vec![atomic_write(vec![repr1, repr2])],
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
    if let JournalResponse::WriteMessageSuccess { .. } = response {
      success_count += 1;
    }
    if let JournalResponse::WriteMessagesSuccessful { instance_id } = response {
      batch_success += 1;
      batch_success_instance_id = Some(*instance_id);
    }
  }
  assert_eq!(success_count, 2);
  assert_eq!(batch_success, 1);
  assert_eq!(batch_success_instance_id, Some(9));
}

#[test]
fn journal_actor_ignores_unrelated_messages() {
  let system = new_test_system();
  let pid = test_actor_pid();
  let mut ctx = ActorContext::new(&system, pid);
  let mut actor = JournalActor::<InMemoryJournal>::new(InMemoryJournal::new());

  let any_message = AnyMessage::new(());
  actor.receive(&mut ctx, any_message.as_view()).expect("receive failed");
}

#[test]
fn should_delegate_unknown_message_to_journal_plugin_handler_when_message_is_not_protocol_message() {
  let system = new_test_system();
  let pid = test_actor_pid();
  let mut ctx = ActorContext::new(&system, pid);
  let markers = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let handler = RecordingJournalPluginHandler::new(markers.clone());
  let mut actor = JournalActor::<InMemoryJournal>::new_with_plugin_handler(InMemoryJournal::new(), handler);

  let any_message = AnyMessage::new(JournalPluginCommand { marker: 42 });
  actor.receive(&mut ctx, any_message.as_view()).expect("receive failed");

  let observed = markers.lock();
  assert_eq!(observed.len(), 1);
  assert_eq!(observed[0], 42);
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
    messages: vec![atomic_write(vec![repr])],
    sender,
    instance_id: 1,
  };

  let any_message = AnyMessage::new(message);
  actor.receive(&mut ctx, any_message.as_view()).expect("receive failed");

  let responses = store.lock();
  assert_eq!(responses.len(), 0);
}

#[test]
fn journal_actor_pending_replay_delete_and_highest_do_not_emit_failure() {
  let system = new_test_system();
  let pid = test_actor_pid();
  let mut ctx = ActorContext::new(&system, pid);
  let config = JournalActorConfig::new(0);
  let mut actor = JournalActor::<PendingJournal>::new_with_config(PendingJournal, config);
  let (sender, store) = create_sender();

  let replay = JournalMessage::ReplayMessages {
    persistence_id:   "pid-1".into(),
    from_sequence_nr: 1,
    to_sequence_nr:   3,
    max:              10,
    sender:           sender.clone(),
  };
  let any_message = AnyMessage::new(replay);
  actor.receive(&mut ctx, any_message.as_view()).expect("replay receive failed");

  let delete = JournalMessage::DeleteMessagesTo {
    persistence_id: "pid-1".into(),
    to_sequence_nr: 3,
    sender:         sender.clone(),
  };
  let any_message = AnyMessage::new(delete);
  actor.receive(&mut ctx, any_message.as_view()).expect("delete receive failed");

  let highest = JournalMessage::GetHighestSequenceNr { persistence_id: "pid-1".into(), from_sequence_nr: 0, sender };
  let any_message = AnyMessage::new(highest);
  actor.receive(&mut ctx, any_message.as_view()).expect("highest receive failed");

  assert!(store.lock().is_empty());
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
    messages: vec![atomic_write(vec![repr])],
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
    if let JournalResponse::WriteMessageFailure { cause, .. } = response {
      assert_eq!(cause, &JournalError::WriteFailed("boom".into()));
      failures += 1;
    }
    if let JournalResponse::WriteMessagesFailed { cause, write_count, instance_id } = response {
      assert_eq!(cause, &JournalError::WriteFailed("boom".into()));
      assert_eq!(*write_count, 1);
      assert_eq!(*instance_id, 1);
      batch_failures += 1;
    }
  }
  assert_eq!(failures, 1);
  assert_eq!(batch_failures, 1);
}

#[test]
fn journal_actor_replay_failure_after_retry_exhausted() {
  let system = new_test_system();
  let pid = test_actor_pid();
  let mut ctx = ActorContext::new(&system, pid);
  let config = JournalActorConfig::new(1);
  let mut actor = JournalActor::<ScriptedJournal>::new_with_config(ScriptedJournal::new(2, 0, 0), config);
  let (sender, store) = create_sender();

  let replay = JournalMessage::ReplayMessages {
    persistence_id: "pid-1".into(),
    from_sequence_nr: 1,
    to_sequence_nr: 3,
    max: 10,
    sender,
  };
  let any_message = AnyMessage::new(replay);
  actor.receive(&mut ctx, any_message.as_view()).expect("replay receive failed");
  assert!(store.lock().is_empty());

  let poll = AnyMessage::new(JournalPoll);
  actor.receive(&mut ctx, poll.as_view()).expect("poll receive failed");

  let responses = store.lock();
  assert_eq!(responses.len(), 1);
  let response = responses[0].payload().downcast_ref::<JournalResponse>().expect("unexpected payload");
  assert!(
    matches!(response, JournalResponse::ReplayMessagesFailure { cause } if cause == &JournalError::ReadFailed("replay failed".into()))
  );
}

#[test]
fn journal_actor_delete_and_highest_success_responses() {
  let system = new_test_system();
  let pid = test_actor_pid();
  let mut ctx = ActorContext::new(&system, pid);
  let mut actor = JournalActor::<InMemoryJournal>::new(InMemoryJournal::new());
  let (sender, store) = create_sender();

  let repr = PersistentRepr::new("pid-1", 1, ArcShared::new(1_i32));
  let write = JournalMessage::WriteMessages {
    persistence_id: "pid-1".into(),
    to_sequence_nr: 1,
    messages:       vec![atomic_write(vec![repr])],
    sender:         sender.clone(),
    instance_id:    7,
  };
  let any_message = AnyMessage::new(write);
  actor.receive(&mut ctx, any_message.as_view()).expect("write receive failed");
  store.lock().clear();

  let delete = JournalMessage::DeleteMessagesTo {
    persistence_id: "pid-1".into(),
    to_sequence_nr: 1,
    sender:         sender.clone(),
  };
  let any_message = AnyMessage::new(delete);
  actor.receive(&mut ctx, any_message.as_view()).expect("delete receive failed");

  let highest = JournalMessage::GetHighestSequenceNr { persistence_id: "pid-1".into(), from_sequence_nr: 0, sender };
  let any_message = AnyMessage::new(highest);
  actor.receive(&mut ctx, any_message.as_view()).expect("highest receive failed");

  let responses = store.lock();
  let mut delete_success = None;
  let mut highest_sequence_nr = None;
  for response in responses.iter() {
    let response = response.payload().downcast_ref::<JournalResponse>().expect("unexpected payload");
    if let JournalResponse::DeleteMessagesSuccess { to_sequence_nr } = response {
      delete_success = Some(*to_sequence_nr);
    }
    if let JournalResponse::HighestSequenceNr { persistence_id, sequence_nr } = response {
      assert_eq!(persistence_id, "pid-1");
      highest_sequence_nr = Some(*sequence_nr);
    }
  }
  assert_eq!(delete_success, Some(1));
  assert_eq!(highest_sequence_nr, Some(1));
}

#[test]
fn journal_actor_delete_failure_after_retry_exhausted() {
  let system = new_test_system();
  let pid = test_actor_pid();
  let mut ctx = ActorContext::new(&system, pid);
  let config = JournalActorConfig::new(1);
  let mut actor = JournalActor::<ScriptedJournal>::new_with_config(ScriptedJournal::new(0, 2, 0), config);
  let (sender, store) = create_sender();

  let delete = JournalMessage::DeleteMessagesTo { persistence_id: "pid-1".into(), to_sequence_nr: 9, sender };
  let any_message = AnyMessage::new(delete);
  actor.receive(&mut ctx, any_message.as_view()).expect("delete receive failed");
  assert!(store.lock().is_empty());

  let poll = AnyMessage::new(JournalPoll);
  actor.receive(&mut ctx, poll.as_view()).expect("poll receive failed");

  let responses = store.lock();
  assert_eq!(responses.len(), 1);
  let response = responses[0].payload().downcast_ref::<JournalResponse>().expect("unexpected payload");
  assert!(matches!(response, JournalResponse::DeleteMessagesFailure { cause, to_sequence_nr }
      if cause == &JournalError::DeleteFailed("delete failed".into()) && *to_sequence_nr == 9));
}

#[test]
fn journal_actor_highest_failure_after_retry_exhausted() {
  let system = new_test_system();
  let pid = test_actor_pid();
  let mut ctx = ActorContext::new(&system, pid);
  let config = JournalActorConfig::new(1);
  let mut actor = JournalActor::<ScriptedJournal>::new_with_config(ScriptedJournal::new(0, 0, 2), config);
  let (sender, store) = create_sender();

  let highest = JournalMessage::GetHighestSequenceNr { persistence_id: "pid-1".into(), from_sequence_nr: 0, sender };
  let any_message = AnyMessage::new(highest);
  actor.receive(&mut ctx, any_message.as_view()).expect("highest receive failed");
  assert!(store.lock().is_empty());

  let poll = AnyMessage::new(JournalPoll);
  actor.receive(&mut ctx, poll.as_view()).expect("poll receive failed");

  let responses = store.lock();
  assert_eq!(responses.len(), 1);
  let response = responses[0].payload().downcast_ref::<JournalResponse>().expect("unexpected payload");
  assert!(matches!(response, JournalResponse::HighestSequenceNrFailure { persistence_id, cause }
      if persistence_id == "pid-1" && cause == &JournalError::ReadFailed("highest failed".into())));
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
    messages:       vec![atomic_write(vec![repr1, repr2, repr3])],
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

#[test]
fn journal_actor_success_responses_to_null_sender_do_not_fail() {
  let system = new_test_system();
  let pid = test_actor_pid();
  let mut ctx = ActorContext::new(&system, pid);
  let mut actor = JournalActor::<InMemoryJournal>::new(InMemoryJournal::new());

  let repr = PersistentRepr::new("pid-1", 1, ArcShared::new(1_i32));
  let write = JournalMessage::WriteMessages {
    persistence_id: "pid-1".into(),
    to_sequence_nr: 1,
    messages:       vec![atomic_write(vec![repr])],
    sender:         ActorRef::null(),
    instance_id:    7,
  };
  let any_message = AnyMessage::new(write);
  actor.receive(&mut ctx, any_message.as_view()).expect("write receive failed");

  let replay = JournalMessage::ReplayMessages {
    persistence_id:   "pid-1".into(),
    from_sequence_nr: 1,
    to_sequence_nr:   1,
    max:              10,
    sender:           ActorRef::null(),
  };
  let any_message = AnyMessage::new(replay);
  actor.receive(&mut ctx, any_message.as_view()).expect("replay receive failed");

  let delete = JournalMessage::DeleteMessagesTo {
    persistence_id: "pid-1".into(),
    to_sequence_nr: 1,
    sender:         ActorRef::null(),
  };
  let any_message = AnyMessage::new(delete);
  actor.receive(&mut ctx, any_message.as_view()).expect("delete receive failed");

  let highest = JournalMessage::GetHighestSequenceNr {
    persistence_id:   "pid-1".into(),
    from_sequence_nr: 0,
    sender:           ActorRef::null(),
  };
  let any_message = AnyMessage::new(highest);
  actor.receive(&mut ctx, any_message.as_view()).expect("highest receive failed");
}

#[test]
fn journal_actor_failure_responses_to_null_sender_do_not_fail() {
  let system = new_test_system();
  let pid = test_actor_pid();
  let mut ctx = ActorContext::new(&system, pid);
  let repr = PersistentRepr::new("pid-1", 1, ArcShared::new(1_i32));
  let write = JournalMessage::WriteMessages {
    persistence_id: "pid-1".into(),
    to_sequence_nr: 1,
    messages:       vec![atomic_write(vec![repr])],
    sender:         ActorRef::null(),
    instance_id:    7,
  };
  let mut write_actor = JournalActor::<RetryJournal>::new_with_config(RetryJournal::new(1), JournalActorConfig::new(0));
  let any_message = AnyMessage::new(write);
  write_actor.receive(&mut ctx, any_message.as_view()).expect("write receive failed");

  let mut actor =
    JournalActor::<ScriptedJournal>::new_with_config(ScriptedJournal::new(1, 1, 1), JournalActorConfig::new(0));
  let replay = JournalMessage::ReplayMessages {
    persistence_id:   "pid-1".into(),
    from_sequence_nr: 1,
    to_sequence_nr:   1,
    max:              10,
    sender:           ActorRef::null(),
  };
  let any_message = AnyMessage::new(replay);
  actor.receive(&mut ctx, any_message.as_view()).expect("replay receive failed");

  let delete = JournalMessage::DeleteMessagesTo {
    persistence_id: "pid-1".into(),
    to_sequence_nr: 1,
    sender:         ActorRef::null(),
  };
  let any_message = AnyMessage::new(delete);
  actor.receive(&mut ctx, any_message.as_view()).expect("delete receive failed");

  let highest = JournalMessage::GetHighestSequenceNr {
    persistence_id:   "pid-1".into(),
    from_sequence_nr: 0,
    sender:           ActorRef::null(),
  };
  let any_message = AnyMessage::new(highest);
  actor.receive(&mut ctx, any_message.as_view()).expect("highest receive failed");
}
