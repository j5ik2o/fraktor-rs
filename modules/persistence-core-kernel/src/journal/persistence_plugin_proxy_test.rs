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
fn should_reply_with_journal_failures_for_each_message_kind_when_journal_target_is_not_set() {
  let system = new_test_system();
  let pid = test_actor_pid();
  let mut ctx = ActorContext::new(&system, pid);
  let mut proxy = PersistencePluginProxyActor::new();
  let (sender, responses) = create_sender(Pid::new(20_010, 1));
  let messages = build_messages("pid-1", 1, 2);
  let write_message = JournalMessage::WriteMessages {
    persistence_id: "pid-1".into(),
    to_sequence_nr: 2,
    messages:       vec![atomic_write(messages)],
    sender:         sender.clone(),
    instance_id:    42,
  };
  let replay_message = JournalMessage::ReplayMessages {
    persistence_id:   "pid-1".into(),
    from_sequence_nr: 1,
    to_sequence_nr:   2,
    max:              10,
    sender:           sender.clone(),
  };
  let delete_message = JournalMessage::DeleteMessagesTo {
    persistence_id: "pid-1".into(),
    to_sequence_nr: 3,
    sender:         sender.clone(),
  };
  let highest_message =
    JournalMessage::GetHighestSequenceNr { persistence_id: "pid-1".into(), from_sequence_nr: 0, sender };

  for message in [write_message, replay_message, delete_message, highest_message] {
    let any_message = AnyMessage::new(message);
    proxy.receive(&mut ctx, any_message.as_view()).expect("journal failure response failed");
  }

  let responses = responses.lock();
  assert_eq!(responses.len(), 6);
  assert!(matches!(
    responses[0].payload().downcast_ref::<JournalResponse>().expect("first write failure"),
    JournalResponse::WriteMessageFailure { instance_id, .. } if *instance_id == 42
  ));
  assert!(matches!(
    responses[1].payload().downcast_ref::<JournalResponse>().expect("second write failure"),
    JournalResponse::WriteMessageFailure { instance_id, .. } if *instance_id == 42
  ));
  assert!(matches!(
    responses[2].payload().downcast_ref::<JournalResponse>().expect("batch write failure"),
    JournalResponse::WriteMessagesFailed { write_count, instance_id, .. } if *write_count == 2 && *instance_id == 42
  ));
  assert!(matches!(
    responses[3].payload().downcast_ref::<JournalResponse>().expect("replay failure"),
    JournalResponse::ReplayMessagesFailure { .. }
  ));
  assert!(matches!(
    responses[4].payload().downcast_ref::<JournalResponse>().expect("delete failure"),
    JournalResponse::DeleteMessagesFailure { to_sequence_nr, .. } if *to_sequence_nr == 3
  ));
  assert!(matches!(
    responses[5].payload().downcast_ref::<JournalResponse>().expect("highest failure"),
    JournalResponse::HighestSequenceNrFailure { persistence_id, .. } if persistence_id == "pid-1"
  ));
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
fn should_reply_with_snapshot_failures_for_each_message_kind_when_snapshot_target_is_not_set() {
  let system = new_test_system();
  let pid = test_actor_pid();
  let mut ctx = ActorContext::new(&system, pid);
  let mut proxy = PersistencePluginProxyActor::new();
  let (sender, responses) = create_sender(Pid::new(20_011, 1));
  let metadata = SnapshotMetadata::new("pid-1", 8, 21);
  let criteria = SnapshotSelectionCriteria::new(8, u64::MAX, 1, 0);
  let save_message =
    SnapshotMessage::SaveSnapshot { metadata: metadata.clone(), snapshot: payload(7), sender: sender.clone() };
  let load_message = SnapshotMessage::LoadSnapshot {
    persistence_id: "pid-1".into(),
    criteria:       criteria.clone(),
    sender:         sender.clone(),
  };
  let delete_message = SnapshotMessage::DeleteSnapshot { metadata: metadata.clone(), sender: sender.clone() };
  let delete_many_message =
    SnapshotMessage::DeleteSnapshots { persistence_id: "pid-1".into(), criteria: criteria.clone(), sender };

  for message in [save_message, load_message, delete_message, delete_many_message] {
    let any_message = AnyMessage::new(message);
    proxy.receive(&mut ctx, any_message.as_view()).expect("snapshot failure response failed");
  }

  let responses = responses.lock();
  assert_eq!(responses.len(), 4);
  assert!(matches!(
    responses[0].payload().downcast_ref::<SnapshotResponse>().expect("save failure"),
    SnapshotResponse::SaveSnapshotFailure { metadata: response_metadata, .. } if response_metadata == &metadata
  ));
  assert!(matches!(
    responses[1].payload().downcast_ref::<SnapshotResponse>().expect("load failure"),
    SnapshotResponse::LoadSnapshotFailed { .. }
  ));
  assert!(matches!(
    responses[2].payload().downcast_ref::<SnapshotResponse>().expect("delete failure"),
    SnapshotResponse::DeleteSnapshotFailure { metadata: response_metadata, .. } if response_metadata == &metadata
  ));
  assert!(matches!(
    responses[3].payload().downcast_ref::<SnapshotResponse>().expect("delete many failure"),
    SnapshotResponse::DeleteSnapshotsFailure { criteria: response_criteria, .. } if response_criteria == &criteria
  ));
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

#[test]
fn should_ignore_failure_reply_delivery_errors() {
  let system = new_test_system();
  let pid = test_actor_pid();
  let mut ctx = ActorContext::new(&system, pid);
  let mut proxy = PersistencePluginProxyActor::new();
  let journal_sender = create_failing_sender(Pid::new(20_012, 1));
  let snapshot_sender = create_failing_sender(Pid::new(20_013, 1));
  let metadata = SnapshotMetadata::new("pid-1", 13, 34);
  let journal_message = JournalMessage::DeleteMessagesTo {
    persistence_id: "pid-1".into(),
    to_sequence_nr: 13,
    sender:         journal_sender,
  };
  let snapshot_message = SnapshotMessage::DeleteSnapshot { metadata, sender: snapshot_sender };

  let any_message = AnyMessage::new(journal_message);
  proxy.receive(&mut ctx, any_message.as_view()).expect("journal delivery failure should be ignored");
  let any_message = AnyMessage::new(snapshot_message);
  proxy.receive(&mut ctx, any_message.as_view()).expect("snapshot delivery failure should be ignored");
}
