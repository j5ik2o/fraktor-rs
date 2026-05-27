use alloc::{boxed::Box, vec::Vec};
use core::{
  any::Any,
  future::Future,
  task::{Context, Poll, Waker},
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
use fraktor_utils_core_rs::sync::{ArcShared, DefaultMutex, SharedLock};

use crate::{
  journal::{
    InMemoryJournal, Journal, JournalMessage, JournalResponse, PersistencePluginProxy, PersistencePluginProxyActor,
    PersistencePluginProxyCommand,
  },
  persistent::{AtomicWrite, PersistentRepr},
  snapshot::{
    InMemorySnapshotStore, SnapshotMessage, SnapshotMetadata, SnapshotResponse, SnapshotSelectionCriteria,
    SnapshotStore,
  },
};

type MessageStore = ArcShared<DefaultMutex<Vec<AnyMessage>>>;

struct TestSender {
  messages: MessageStore,
}

impl ActorRefSender for TestSender {
  fn send(&mut self, message: AnyMessage) -> Result<SendOutcome, SendError> {
    self.messages.lock().push(message);
    Ok(SendOutcome::Delivered)
  }
}

struct FailingSender;

impl ActorRefSender for FailingSender {
  fn send(&mut self, message: AnyMessage) -> Result<SendOutcome, SendError> {
    Err(SendError::closed(message))
  }
}

fn actor_ref_with_sender(pid: Pid, sender: impl ActorRefSender + 'static) -> ActorRef {
  let sender = ActorRefSenderShared::from_shared_lock(SharedLock::new_with_driver::<
    DefaultMutex<Box<dyn ActorRefSender>>,
  >(Box::new(sender)));
  ActorRef::new(pid, sender)
}

fn create_sender(pid: Pid) -> (ActorRef, MessageStore) {
  let messages = ArcShared::new(DefaultMutex::new(Vec::new()));
  let sender = actor_ref_with_sender(pid, TestSender { messages: messages.clone() });
  (sender, messages)
}

fn create_failing_sender(pid: Pid) -> ActorRef {
  actor_ref_with_sender(pid, FailingSender)
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

fn poll_ready<F: Future>(future: F) -> F::Output {
  let waker = Waker::noop();
  let mut cx = Context::from_waker(waker);
  let mut future = Box::pin(future);
  match Future::poll(future.as_mut(), &mut cx) {
    | Poll::Ready(output) => output,
    | Poll::Pending => panic!("future was pending"),
  }
}

fn build_messages(persistence_id: &str, start: u64, count: u64) -> Vec<PersistentRepr> {
  (0..count)
    .map(|offset| {
      let payload: ArcShared<dyn Any + Send + Sync> = ArcShared::new((start + offset) as i32);
      PersistentRepr::new(persistence_id, start + offset, payload)
    })
    .collect()
}

fn atomic_write(payload: Vec<PersistentRepr>) -> AtomicWrite {
  AtomicWrite::new(payload).expect("atomic write")
}

fn payload(value: i32) -> ArcShared<dyn Any + Send + Sync> {
  ArcShared::new(value)
}

#[test]
fn plugin_proxy_forwards_journal_operations() {
  let mut proxy = PersistencePluginProxy::new(InMemoryJournal::new(), InMemorySnapshotStore::new());
  let messages = build_messages("pid-1", 1, 2);

  poll_ready(Journal::write_messages(&mut proxy, &[atomic_write(messages)])).expect("write failed");
  let replayed = poll_ready(Journal::replay_messages(&proxy, "pid-1", 1, 10, 10)).expect("replay failed");
  let highest = poll_ready(Journal::highest_sequence_nr(&proxy, "pid-1")).expect("highest failed");

  assert_eq!(replayed.len(), 2);
  assert_eq!(highest, 2);
}

#[test]
fn plugin_proxy_forwards_snapshot_operations() {
  let mut proxy = PersistencePluginProxy::new(InMemoryJournal::new(), InMemorySnapshotStore::new());
  let metadata = SnapshotMetadata::new("pid-1", 1, 10);

  poll_ready(SnapshotStore::save_snapshot(&mut proxy, metadata.clone(), payload(7))).expect("save failed");
  let loaded = poll_ready(SnapshotStore::load_snapshot(&proxy, "pid-1", SnapshotSelectionCriteria::latest()))
    .expect("load failed");
  let snapshot = loaded.expect("snapshot should exist");

  assert_eq!(snapshot.metadata(), &metadata);
}

#[test]
fn plugin_proxy_set_target_replaces_plugins() {
  let mut proxy = PersistencePluginProxy::new(InMemoryJournal::new(), InMemorySnapshotStore::new());
  let messages = build_messages("pid-1", 1, 1);
  poll_ready(Journal::write_messages(&mut proxy, &[atomic_write(messages)])).expect("write failed");

  proxy.set_target(InMemoryJournal::new(), InMemorySnapshotStore::new());

  let replayed = poll_ready(Journal::replay_messages(&proxy, "pid-1", 1, 10, 10)).expect("replay failed");
  let highest = poll_ready(Journal::highest_sequence_nr(&proxy, "pid-1")).expect("highest failed");
  let loaded = poll_ready(SnapshotStore::load_snapshot(&proxy, "pid-1", SnapshotSelectionCriteria::latest()))
    .expect("load failed");

  assert!(replayed.is_empty());
  assert_eq!(highest, 0);
  assert!(loaded.is_none());
}

#[test]
fn should_forward_journal_message_to_configured_target_when_journal_target_is_set() {
  let system = new_test_system();
  let pid = test_actor_pid();
  let mut ctx = ActorContext::new(&system, pid);
  let mut proxy = PersistencePluginProxyActor::new();
  let (target, target_store) = create_sender(Pid::new(20_000, 1));
  let command = PersistencePluginProxyCommand::SetJournalTarget { target };
  let any_message = AnyMessage::new(command);
  proxy.receive(&mut ctx, any_message.as_view()).expect("set journal target failed");
  let (sender, _responses) = create_sender(Pid::new(20_001, 1));

  let message = JournalMessage::DeleteMessagesTo { persistence_id: "pid-1".into(), to_sequence_nr: 7, sender };
  let any_message = AnyMessage::new(message);
  proxy.receive(&mut ctx, any_message.as_view()).expect("journal forwarding failed");

  let forwarded = target_store.lock();
  assert_eq!(forwarded.len(), 1);
  let message = forwarded[0].payload().downcast_ref::<JournalMessage>().expect("unexpected payload");
  assert!(matches!(message, JournalMessage::DeleteMessagesTo { persistence_id, to_sequence_nr, .. }
      if persistence_id == "pid-1" && *to_sequence_nr == 7));
}

#[test]
fn should_forward_snapshot_message_to_configured_target_when_snapshot_target_is_set() {
  let system = new_test_system();
  let pid = test_actor_pid();
  let mut ctx = ActorContext::new(&system, pid);
  let mut proxy = PersistencePluginProxyActor::new();
  let (target, target_store) = create_sender(Pid::new(20_002, 1));
  let command = PersistencePluginProxyCommand::SetSnapshotTarget { target };
  let any_message = AnyMessage::new(command);
  proxy.receive(&mut ctx, any_message.as_view()).expect("set snapshot target failed");
  let (sender, _responses) = create_sender(Pid::new(20_003, 1));
  let metadata = SnapshotMetadata::new("pid-1", 3, 11);

  let message = SnapshotMessage::DeleteSnapshot { metadata: metadata.clone(), sender };
  let any_message = AnyMessage::new(message);
  proxy.receive(&mut ctx, any_message.as_view()).expect("snapshot forwarding failed");

  let forwarded = target_store.lock();
  assert_eq!(forwarded.len(), 1);
  let message = forwarded[0].payload().downcast_ref::<SnapshotMessage>().expect("unexpected payload");
  assert!(matches!(message, SnapshotMessage::DeleteSnapshot { metadata: response_metadata, .. }
      if response_metadata == &metadata));
}

#[test]
fn should_reply_with_journal_failure_when_journal_target_is_not_set() {
  let system = new_test_system();
  let pid = test_actor_pid();
  let mut ctx = ActorContext::new(&system, pid);
  let mut proxy = PersistencePluginProxyActor::new();
  let (sender, responses) = create_sender(Pid::new(20_004, 1));

  let message = JournalMessage::DeleteMessagesTo { persistence_id: "pid-1".into(), to_sequence_nr: 9, sender };
  let any_message = AnyMessage::new(message);
  proxy.receive(&mut ctx, any_message.as_view()).expect("journal failure response failed");

  let responses = responses.lock();
  assert_eq!(responses.len(), 1);
  let response = responses[0].payload().downcast_ref::<JournalResponse>().expect("unexpected payload");
  assert!(matches!(response, JournalResponse::DeleteMessagesFailure { to_sequence_nr, .. } if *to_sequence_nr == 9));
}

#[test]
fn should_reply_with_journal_failure_when_journal_target_forwarding_fails() {
  let system = new_test_system();
  let pid = test_actor_pid();
  let mut ctx = ActorContext::new(&system, pid);
  let mut proxy = PersistencePluginProxyActor::new();
  let target = create_failing_sender(Pid::new(20_006, 1));
  let command = PersistencePluginProxyCommand::SetJournalTarget { target };
  let any_message = AnyMessage::new(command);
  proxy.receive(&mut ctx, any_message.as_view()).expect("set journal target failed");
  let (sender, responses) = create_sender(Pid::new(20_007, 1));

  let message = JournalMessage::DeleteMessagesTo { persistence_id: "pid-1".into(), to_sequence_nr: 11, sender };
  let any_message = AnyMessage::new(message);
  proxy.receive(&mut ctx, any_message.as_view()).expect("journal forwarding failure response failed");

  let responses = responses.lock();
  assert_eq!(responses.len(), 1);
  let response = responses[0].payload().downcast_ref::<JournalResponse>().expect("unexpected payload");
  assert!(matches!(response, JournalResponse::DeleteMessagesFailure { to_sequence_nr, .. } if *to_sequence_nr == 11));
}

#[test]
fn should_reply_with_snapshot_failure_when_snapshot_target_is_not_set() {
  let system = new_test_system();
  let pid = test_actor_pid();
  let mut ctx = ActorContext::new(&system, pid);
  let mut proxy = PersistencePluginProxyActor::new();
  let (sender, responses) = create_sender(Pid::new(20_005, 1));
  let metadata = SnapshotMetadata::new("pid-1", 5, 13);

  let message = SnapshotMessage::DeleteSnapshot { metadata: metadata.clone(), sender };
  let any_message = AnyMessage::new(message);
  proxy.receive(&mut ctx, any_message.as_view()).expect("snapshot failure response failed");

  let responses = responses.lock();
  assert_eq!(responses.len(), 1);
  let response = responses[0].payload().downcast_ref::<SnapshotResponse>().expect("unexpected payload");
  assert!(matches!(response, SnapshotResponse::DeleteSnapshotFailure { metadata: response_metadata, .. }
      if response_metadata == &metadata));
}

#[test]
fn should_reply_with_snapshot_failure_when_snapshot_target_forwarding_fails() {
  let system = new_test_system();
  let pid = test_actor_pid();
  let mut ctx = ActorContext::new(&system, pid);
  let mut proxy = PersistencePluginProxyActor::new();
  let target = create_failing_sender(Pid::new(20_008, 1));
  let command = PersistencePluginProxyCommand::SetSnapshotTarget { target };
  let any_message = AnyMessage::new(command);
  proxy.receive(&mut ctx, any_message.as_view()).expect("set snapshot target failed");
  let (sender, responses) = create_sender(Pid::new(20_009, 1));
  let metadata = SnapshotMetadata::new("pid-1", 8, 21);

  let message = SnapshotMessage::DeleteSnapshot { metadata: metadata.clone(), sender };
  let any_message = AnyMessage::new(message);
  proxy.receive(&mut ctx, any_message.as_view()).expect("snapshot forwarding failure response failed");

  let responses = responses.lock();
  assert_eq!(responses.len(), 1);
  let response = responses[0].payload().downcast_ref::<SnapshotResponse>().expect("unexpected payload");
  assert!(matches!(response, SnapshotResponse::DeleteSnapshotFailure { metadata: response_metadata, .. }
      if response_metadata == &metadata));
}
