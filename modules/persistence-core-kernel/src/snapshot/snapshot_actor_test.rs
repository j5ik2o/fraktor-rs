use core::{
  any::Any,
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

use super::SnapshotPoll;
use crate::{
  PluginMessageHandling,
  snapshot::{
    InMemorySnapshotStore, Snapshot, SnapshotActor, SnapshotActorConfig, SnapshotError, SnapshotMessage,
    SnapshotMetadata, SnapshotPluginMessageHandler, SnapshotResponse, SnapshotSelectionCriteria,
  },
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

struct PendingSnapshotStore;

impl crate::snapshot::SnapshotStore for PendingSnapshotStore {
  type DeleteManyFuture<'a>
    = Pending<Result<(), SnapshotError>>
  where
    Self: 'a;
  type DeleteOneFuture<'a>
    = Pending<Result<(), SnapshotError>>
  where
    Self: 'a;
  type LoadFuture<'a>
    = Pending<Result<Option<Snapshot>, SnapshotError>>
  where
    Self: 'a;
  type SaveFuture<'a>
    = Pending<Result<(), SnapshotError>>
  where
    Self: 'a;

  fn save_snapshot<'a>(
    &'a mut self,
    _metadata: SnapshotMetadata,
    _snapshot: ArcShared<dyn Any + Send + Sync>,
  ) -> Self::SaveFuture<'a> {
    pending()
  }

  fn load_snapshot<'a>(
    &'a self,
    _persistence_id: &'a str,
    _criteria: SnapshotSelectionCriteria,
  ) -> Self::LoadFuture<'a> {
    pending()
  }

  fn delete_snapshot<'a>(&'a mut self, _metadata: &'a SnapshotMetadata) -> Self::DeleteOneFuture<'a> {
    pending()
  }

  fn delete_snapshots<'a>(
    &'a mut self,
    _persistence_id: &'a str,
    _criteria: SnapshotSelectionCriteria,
  ) -> Self::DeleteManyFuture<'a> {
    pending()
  }
}

struct RetrySnapshotStore {
  failures_left: u32,
}

impl RetrySnapshotStore {
  fn new(failures_left: u32) -> Self {
    Self { failures_left }
  }
}

impl crate::snapshot::SnapshotStore for RetrySnapshotStore {
  type DeleteManyFuture<'a>
    = Ready<Result<(), SnapshotError>>
  where
    Self: 'a;
  type DeleteOneFuture<'a>
    = Ready<Result<(), SnapshotError>>
  where
    Self: 'a;
  type LoadFuture<'a>
    = Ready<Result<Option<Snapshot>, SnapshotError>>
  where
    Self: 'a;
  type SaveFuture<'a>
    = Ready<Result<(), SnapshotError>>
  where
    Self: 'a;

  fn save_snapshot<'a>(
    &'a mut self,
    _metadata: SnapshotMetadata,
    _snapshot: ArcShared<dyn Any + Send + Sync>,
  ) -> Self::SaveFuture<'a> {
    if self.failures_left > 0 {
      self.failures_left -= 1;
      ready(Err(SnapshotError::SaveFailed("boom".into())))
    } else {
      ready(Ok(()))
    }
  }

  fn load_snapshot<'a>(
    &'a self,
    _persistence_id: &'a str,
    _criteria: SnapshotSelectionCriteria,
  ) -> Self::LoadFuture<'a> {
    ready(Ok(None))
  }

  fn delete_snapshot<'a>(&'a mut self, _metadata: &'a SnapshotMetadata) -> Self::DeleteOneFuture<'a> {
    ready(Ok(()))
  }

  fn delete_snapshots<'a>(
    &'a mut self,
    _persistence_id: &'a str,
    _criteria: SnapshotSelectionCriteria,
  ) -> Self::DeleteManyFuture<'a> {
    ready(Ok(()))
  }
}

struct ScriptedSnapshotStore {
  load_failures_left:        AtomicU32,
  delete_one_failures_left:  AtomicU32,
  delete_many_failures_left: AtomicU32,
}

impl ScriptedSnapshotStore {
  fn new(load_failures_left: u32, delete_one_failures_left: u32, delete_many_failures_left: u32) -> Self {
    Self {
      load_failures_left:        AtomicU32::new(load_failures_left),
      delete_one_failures_left:  AtomicU32::new(delete_one_failures_left),
      delete_many_failures_left: AtomicU32::new(delete_many_failures_left),
    }
  }

  fn consume_failure(counter: &AtomicU32) -> bool {
    counter.fetch_update(Ordering::AcqRel, Ordering::Acquire, |value| value.checked_sub(1)).is_ok()
  }
}

impl crate::snapshot::SnapshotStore for ScriptedSnapshotStore {
  type DeleteManyFuture<'a>
    = Ready<Result<(), SnapshotError>>
  where
    Self: 'a;
  type DeleteOneFuture<'a>
    = Ready<Result<(), SnapshotError>>
  where
    Self: 'a;
  type LoadFuture<'a>
    = Ready<Result<Option<Snapshot>, SnapshotError>>
  where
    Self: 'a;
  type SaveFuture<'a>
    = Ready<Result<(), SnapshotError>>
  where
    Self: 'a;

  fn save_snapshot<'a>(
    &'a mut self,
    _metadata: SnapshotMetadata,
    _snapshot: ArcShared<dyn Any + Send + Sync>,
  ) -> Self::SaveFuture<'a> {
    ready(Ok(()))
  }

  fn load_snapshot<'a>(
    &'a self,
    _persistence_id: &'a str,
    _criteria: SnapshotSelectionCriteria,
  ) -> Self::LoadFuture<'a> {
    if Self::consume_failure(&self.load_failures_left) {
      ready(Err(SnapshotError::LoadFailed("load failed".into())))
    } else {
      ready(Ok(None))
    }
  }

  fn delete_snapshot<'a>(&'a mut self, _metadata: &'a SnapshotMetadata) -> Self::DeleteOneFuture<'a> {
    if Self::consume_failure(&self.delete_one_failures_left) {
      ready(Err(SnapshotError::DeleteFailed("delete one failed".into())))
    } else {
      ready(Ok(()))
    }
  }

  fn delete_snapshots<'a>(
    &'a mut self,
    _persistence_id: &'a str,
    _criteria: SnapshotSelectionCriteria,
  ) -> Self::DeleteManyFuture<'a> {
    if Self::consume_failure(&self.delete_many_failures_left) {
      ready(Err(SnapshotError::DeleteFailed("delete many failed".into())))
    } else {
      ready(Ok(()))
    }
  }
}

struct RecordingSnapshotPluginHandler {
  responses: ArcShared<SpinSyncMutex<Vec<SnapshotResponse>>>,
  markers:   ArcShared<SpinSyncMutex<Vec<u32>>>,
}

impl RecordingSnapshotPluginHandler {
  fn new(responses: ArcShared<SpinSyncMutex<Vec<SnapshotResponse>>>) -> Self {
    Self { responses, markers: ArcShared::new(SpinSyncMutex::new(Vec::new())) }
  }

  fn with_markers(markers: ArcShared<SpinSyncMutex<Vec<u32>>>) -> Self {
    Self { responses: ArcShared::new(SpinSyncMutex::new(Vec::new())), markers }
  }
}

impl SnapshotPluginMessageHandler for RecordingSnapshotPluginHandler {
  fn handle_snapshot_plugin_message(
    &mut self,
    _ctx: &mut ActorContext<'_>,
    message: AnyMessageView<'_>,
  ) -> Result<PluginMessageHandling, ActorError> {
    if let Some(response) = message.downcast_ref::<SnapshotResponse>() {
      self.responses.lock().push(response.clone());
      return Ok(PluginMessageHandling::Handled);
    }
    if let Some(command) = message.downcast_ref::<SnapshotPluginCommand>() {
      self.markers.lock().push(command.marker);
      return Ok(PluginMessageHandling::Handled);
    }
    Ok(PluginMessageHandling::Unhandled)
  }
}

struct SnapshotPluginCommand {
  marker: u32,
}

#[test]
fn scripted_snapshot_store_success_paths_are_exercised() {
  let system = new_test_system();
  let pid = test_actor_pid();
  let mut ctx = ActorContext::new(&system, pid);
  let mut actor = SnapshotActor::<ScriptedSnapshotStore>::new_with_config(
    ScriptedSnapshotStore::new(0, 0, 0),
    SnapshotActorConfig::new(0),
  );
  let (sender, store) = create_sender();
  let metadata = SnapshotMetadata::new("pid-1", 1, 10);

  let save = SnapshotMessage::SaveSnapshot {
    metadata: metadata.clone(),
    snapshot: ArcShared::new(1_i32),
    sender:   sender.clone(),
  };
  let any_message = AnyMessage::new(save);
  actor.receive(&mut ctx, any_message.as_view()).expect("save receive failed");

  let load = SnapshotMessage::LoadSnapshot {
    persistence_id: "pid-1".into(),
    criteria:       SnapshotSelectionCriteria::latest(),
    sender:         sender.clone(),
  };
  let any_message = AnyMessage::new(load);
  actor.receive(&mut ctx, any_message.as_view()).expect("load receive failed");

  let delete_one = SnapshotMessage::DeleteSnapshot { metadata, sender: sender.clone() };
  let any_message = AnyMessage::new(delete_one);
  actor.receive(&mut ctx, any_message.as_view()).expect("delete one receive failed");

  let delete_many = SnapshotMessage::DeleteSnapshots {
    persistence_id: "pid-1".into(),
    criteria: SnapshotSelectionCriteria::latest(),
    sender,
  };
  let any_message = AnyMessage::new(delete_many);
  actor.receive(&mut ctx, any_message.as_view()).expect("delete many receive failed");

  let responses = store.lock();
  assert_eq!(responses.len(), 4);
}

#[test]
fn snapshot_actor_save_and_load_responses() {
  let system = new_test_system();
  let pid = test_actor_pid();
  let mut ctx = ActorContext::new(&system, pid);
  let mut actor = SnapshotActor::<InMemorySnapshotStore>::new(InMemorySnapshotStore::new());
  let (sender, store) = create_sender();
  let metadata = SnapshotMetadata::new("pid-1", 1, 10);
  let payload = ArcShared::new(1_i32);

  let save = SnapshotMessage::SaveSnapshot { metadata: metadata.clone(), snapshot: payload, sender: sender.clone() };
  let any_message = AnyMessage::new(save);
  actor.receive(&mut ctx, any_message.as_view()).expect("receive failed");

  {
    let responses = store.lock();
    assert_eq!(responses.len(), 1);
    let response = responses[0].payload().downcast_ref::<SnapshotResponse>().expect("unexpected payload");
    match response {
      | SnapshotResponse::SaveSnapshotSuccess { metadata: response_metadata } => {
        assert_eq!(response_metadata.sequence_nr(), 1);
      },
      | _ => panic!("unexpected response"),
    }
  }

  store.lock().clear();

  let load = SnapshotMessage::LoadSnapshot {
    persistence_id: "pid-1".into(),
    criteria: SnapshotSelectionCriteria::latest(),
    sender,
  };
  let any_message = AnyMessage::new(load);
  actor.receive(&mut ctx, any_message.as_view()).expect("receive failed");

  let responses = store.lock();
  assert_eq!(responses.len(), 1);
  let response = responses[0].payload().downcast_ref::<SnapshotResponse>().expect("unexpected payload");
  assert!(matches!(response, SnapshotResponse::LoadSnapshotResult {
    snapshot:       Some(_),
    to_sequence_nr: u64::MAX,
  }));
}

#[test]
fn snapshot_actor_ignores_unrelated_messages() {
  let system = new_test_system();
  let pid = test_actor_pid();
  let mut ctx = ActorContext::new(&system, pid);
  let mut actor = SnapshotActor::<InMemorySnapshotStore>::new(InMemorySnapshotStore::new());

  let any_message = AnyMessage::new(());
  actor.receive(&mut ctx, any_message.as_view()).expect("receive failed");
}

#[test]
fn should_observe_snapshot_completion_response_in_plugin_handler_when_save_finishes() {
  let system = new_test_system();
  let pid = test_actor_pid();
  let mut ctx = ActorContext::new(&system, pid);
  let responses = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let handler = RecordingSnapshotPluginHandler::new(responses.clone());
  let mut actor =
    SnapshotActor::<InMemorySnapshotStore>::new_with_plugin_handler(InMemorySnapshotStore::new(), handler);
  let (sender, _store) = create_sender();
  let metadata = SnapshotMetadata::new("pid-1", 1, 10);

  let save = SnapshotMessage::SaveSnapshot { metadata: metadata.clone(), snapshot: ArcShared::new(1_i32), sender };
  let any_message = AnyMessage::new(save);
  actor.receive(&mut ctx, any_message.as_view()).expect("receive failed");

  let observed = responses.lock();
  assert_eq!(observed.len(), 1);
  assert!(matches!(
    &observed[0],
    SnapshotResponse::SaveSnapshotSuccess { metadata: response_metadata }
      if response_metadata == &metadata
  ));
}

#[test]
fn should_delegate_unknown_message_to_snapshot_plugin_handler_when_message_is_not_protocol_message() {
  let system = new_test_system();
  let pid = test_actor_pid();
  let mut ctx = ActorContext::new(&system, pid);
  let markers = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let handler = RecordingSnapshotPluginHandler::with_markers(markers.clone());
  let mut actor =
    SnapshotActor::<InMemorySnapshotStore>::new_with_plugin_handler(InMemorySnapshotStore::new(), handler);

  let any_message = AnyMessage::new(SnapshotPluginCommand { marker: 42 });
  actor.receive(&mut ctx, any_message.as_view()).expect("receive failed");

  let observed = markers.lock();
  assert_eq!(observed.len(), 1);
  assert_eq!(observed[0], 42);
}

#[test]
fn snapshot_actor_pending_does_not_emit_failure() {
  let system = new_test_system();
  let pid = test_actor_pid();
  let mut ctx = ActorContext::new(&system, pid);
  let config = SnapshotActorConfig::new(0);
  let mut actor = SnapshotActor::<PendingSnapshotStore>::new_with_config(PendingSnapshotStore, config);
  let (sender, store) = create_sender();

  let load = SnapshotMessage::LoadSnapshot {
    persistence_id: "pid-1".into(),
    criteria: SnapshotSelectionCriteria::latest(),
    sender,
  };
  let any_message = AnyMessage::new(load);
  actor.receive(&mut ctx, any_message.as_view()).expect("receive failed");

  let responses = store.lock();
  assert_eq!(responses.len(), 0);
}

#[test]
fn snapshot_actor_pending_save_delete_one_and_delete_many_do_not_emit_failure() {
  let system = new_test_system();
  let pid = test_actor_pid();
  let mut ctx = ActorContext::new(&system, pid);
  let config = SnapshotActorConfig::new(0);
  let mut actor = SnapshotActor::<PendingSnapshotStore>::new_with_config(PendingSnapshotStore, config);
  let (sender, store) = create_sender();
  let metadata = SnapshotMetadata::new("pid-1", 1, 10);

  let save = SnapshotMessage::SaveSnapshot {
    metadata: metadata.clone(),
    snapshot: ArcShared::new(1_i32),
    sender:   sender.clone(),
  };
  let any_message = AnyMessage::new(save);
  actor.receive(&mut ctx, any_message.as_view()).expect("save receive failed");

  let delete_one = SnapshotMessage::DeleteSnapshot { metadata, sender: sender.clone() };
  let any_message = AnyMessage::new(delete_one);
  actor.receive(&mut ctx, any_message.as_view()).expect("delete one receive failed");

  let delete_many = SnapshotMessage::DeleteSnapshots {
    persistence_id: "pid-1".into(),
    criteria: SnapshotSelectionCriteria::latest(),
    sender,
  };
  let any_message = AnyMessage::new(delete_many);
  actor.receive(&mut ctx, any_message.as_view()).expect("delete many receive failed");

  assert!(store.lock().is_empty());
}

#[test]
fn snapshot_actor_retry_max_exceeded_on_errors() {
  let system = new_test_system();
  let pid = test_actor_pid();
  let mut ctx = ActorContext::new(&system, pid);
  let config = SnapshotActorConfig::new(1);
  let mut actor = SnapshotActor::<RetrySnapshotStore>::new_with_config(RetrySnapshotStore::new(2), config);
  let (sender, store) = create_sender();
  let metadata = SnapshotMetadata::new("pid-1", 1, 10);
  let payload = ArcShared::new(1_i32);

  let save = SnapshotMessage::SaveSnapshot { metadata: metadata.clone(), snapshot: payload, sender };
  let any_message = AnyMessage::new(save);
  actor.receive(&mut ctx, any_message.as_view()).expect("receive failed");

  assert!(store.lock().is_empty());

  let poll = AnyMessage::new(SnapshotPoll);
  actor.receive(&mut ctx, poll.as_view()).expect("receive failed");

  let responses = store.lock();
  assert_eq!(responses.len(), 1);
  let response = responses[0].payload().downcast_ref::<SnapshotResponse>().expect("unexpected payload");
  assert!(matches!(
    response,
    SnapshotResponse::SaveSnapshotFailure { error, .. }
      if *error == SnapshotError::SaveFailed("boom".into())
  ));
}

#[test]
fn snapshot_actor_delete_one_and_delete_many_success_responses() {
  let system = new_test_system();
  let pid = test_actor_pid();
  let mut ctx = ActorContext::new(&system, pid);
  let mut actor = SnapshotActor::<InMemorySnapshotStore>::new(InMemorySnapshotStore::new());
  let (sender, store) = create_sender();
  let metadata = SnapshotMetadata::new("pid-1", 1, 10);

  let save = SnapshotMessage::SaveSnapshot {
    metadata: metadata.clone(),
    snapshot: ArcShared::new(1_i32),
    sender:   sender.clone(),
  };
  let any_message = AnyMessage::new(save);
  actor.receive(&mut ctx, any_message.as_view()).expect("save receive failed");
  store.lock().clear();

  let delete_one = SnapshotMessage::DeleteSnapshot { metadata: metadata.clone(), sender: sender.clone() };
  let any_message = AnyMessage::new(delete_one);
  actor.receive(&mut ctx, any_message.as_view()).expect("delete one receive failed");

  let delete_many = SnapshotMessage::DeleteSnapshots {
    persistence_id: "pid-1".into(),
    criteria: SnapshotSelectionCriteria::latest(),
    sender,
  };
  let any_message = AnyMessage::new(delete_many);
  actor.receive(&mut ctx, any_message.as_view()).expect("delete many receive failed");

  let responses = store.lock();
  let mut delete_one_success = false;
  let mut delete_many_success = false;
  for response in responses.iter() {
    let response = response.payload().downcast_ref::<SnapshotResponse>().expect("unexpected payload");
    if let SnapshotResponse::DeleteSnapshotSuccess { metadata: response_metadata } = response {
      assert_eq!(response_metadata.sequence_nr(), 1);
      delete_one_success = true;
    }
    if let SnapshotResponse::DeleteSnapshotsSuccess { criteria } = response {
      assert_eq!(criteria.max_sequence_nr(), u64::MAX);
      delete_many_success = true;
    }
  }
  assert!(delete_one_success);
  assert!(delete_many_success);
}

#[test]
fn snapshot_actor_load_failure_after_retry_exhausted() {
  let system = new_test_system();
  let pid = test_actor_pid();
  let mut ctx = ActorContext::new(&system, pid);
  let config = SnapshotActorConfig::new(1);
  let mut actor = SnapshotActor::<ScriptedSnapshotStore>::new_with_config(ScriptedSnapshotStore::new(2, 0, 0), config);
  let (sender, store) = create_sender();

  let load = SnapshotMessage::LoadSnapshot {
    persistence_id: "pid-1".into(),
    criteria: SnapshotSelectionCriteria::latest(),
    sender,
  };
  let any_message = AnyMessage::new(load);
  actor.receive(&mut ctx, any_message.as_view()).expect("load receive failed");
  assert!(store.lock().is_empty());

  let poll = AnyMessage::new(SnapshotPoll);
  actor.receive(&mut ctx, poll.as_view()).expect("poll receive failed");

  let responses = store.lock();
  assert_eq!(responses.len(), 1);
  let response = responses[0].payload().downcast_ref::<SnapshotResponse>().expect("unexpected payload");
  assert!(
    matches!(response, SnapshotResponse::LoadSnapshotFailed { error } if error == &SnapshotError::LoadFailed("load failed".into()))
  );
}

#[test]
fn snapshot_actor_delete_one_failure_after_retry_exhausted() {
  let system = new_test_system();
  let pid = test_actor_pid();
  let mut ctx = ActorContext::new(&system, pid);
  let config = SnapshotActorConfig::new(1);
  let mut actor = SnapshotActor::<ScriptedSnapshotStore>::new_with_config(ScriptedSnapshotStore::new(0, 2, 0), config);
  let (sender, store) = create_sender();

  let metadata = SnapshotMetadata::new("pid-1", 1, 10);
  let delete_one = SnapshotMessage::DeleteSnapshot { metadata, sender };
  let any_message = AnyMessage::new(delete_one);
  actor.receive(&mut ctx, any_message.as_view()).expect("delete one receive failed");
  assert!(store.lock().is_empty());

  let poll = AnyMessage::new(SnapshotPoll);
  actor.receive(&mut ctx, poll.as_view()).expect("poll receive failed");

  let responses = store.lock();
  assert_eq!(responses.len(), 1);
  let response = responses[0].payload().downcast_ref::<SnapshotResponse>().expect("unexpected payload");
  assert!(matches!(response, SnapshotResponse::DeleteSnapshotFailure { metadata, error }
      if metadata.sequence_nr() == 1 && error == &SnapshotError::DeleteFailed("delete one failed".into())));
}

#[test]
fn snapshot_actor_delete_many_failure_after_retry_exhausted() {
  let system = new_test_system();
  let pid = test_actor_pid();
  let mut ctx = ActorContext::new(&system, pid);
  let config = SnapshotActorConfig::new(1);
  let mut actor = SnapshotActor::<ScriptedSnapshotStore>::new_with_config(ScriptedSnapshotStore::new(0, 0, 2), config);
  let (sender, store) = create_sender();

  let delete_many = SnapshotMessage::DeleteSnapshots {
    persistence_id: "pid-1".into(),
    criteria: SnapshotSelectionCriteria::latest(),
    sender,
  };
  let any_message = AnyMessage::new(delete_many);
  actor.receive(&mut ctx, any_message.as_view()).expect("delete many receive failed");
  assert!(store.lock().is_empty());

  let poll = AnyMessage::new(SnapshotPoll);
  actor.receive(&mut ctx, poll.as_view()).expect("poll receive failed");

  let responses = store.lock();
  assert_eq!(responses.len(), 1);
  let response = responses[0].payload().downcast_ref::<SnapshotResponse>().expect("unexpected payload");
  assert!(matches!(response, SnapshotResponse::DeleteSnapshotsFailure { criteria, error }
      if criteria.max_sequence_nr() == u64::MAX && error == &SnapshotError::DeleteFailed("delete many failed".into())));
}

#[test]
fn snapshot_actor_success_responses_to_null_sender_do_not_fail() {
  let system = new_test_system();
  let pid = test_actor_pid();
  let mut ctx = ActorContext::new(&system, pid);
  let mut actor = SnapshotActor::<InMemorySnapshotStore>::new(InMemorySnapshotStore::new());
  let metadata = SnapshotMetadata::new("pid-1", 1, 10);

  let save = SnapshotMessage::SaveSnapshot {
    metadata: metadata.clone(),
    snapshot: ArcShared::new(1_i32),
    sender:   ActorRef::null(),
  };
  let any_message = AnyMessage::new(save);
  actor.receive(&mut ctx, any_message.as_view()).expect("save receive failed");

  let load = SnapshotMessage::LoadSnapshot {
    persistence_id: "pid-1".into(),
    criteria:       SnapshotSelectionCriteria::latest(),
    sender:         ActorRef::null(),
  };
  let any_message = AnyMessage::new(load);
  actor.receive(&mut ctx, any_message.as_view()).expect("load receive failed");

  let delete_one = SnapshotMessage::DeleteSnapshot { metadata, sender: ActorRef::null() };
  let any_message = AnyMessage::new(delete_one);
  actor.receive(&mut ctx, any_message.as_view()).expect("delete one receive failed");

  let delete_many = SnapshotMessage::DeleteSnapshots {
    persistence_id: "pid-1".into(),
    criteria:       SnapshotSelectionCriteria::latest(),
    sender:         ActorRef::null(),
  };
  let any_message = AnyMessage::new(delete_many);
  actor.receive(&mut ctx, any_message.as_view()).expect("delete many receive failed");
}

#[test]
fn snapshot_actor_failure_responses_to_null_sender_do_not_fail() {
  let system = new_test_system();
  let pid = test_actor_pid();
  let mut ctx = ActorContext::new(&system, pid);
  let metadata = SnapshotMetadata::new("pid-1", 1, 10);
  let save = SnapshotMessage::SaveSnapshot {
    metadata: metadata.clone(),
    snapshot: ArcShared::new(1_i32),
    sender:   ActorRef::null(),
  };
  let mut save_actor =
    SnapshotActor::<RetrySnapshotStore>::new_with_config(RetrySnapshotStore::new(1), SnapshotActorConfig::new(0));
  let any_message = AnyMessage::new(save);
  save_actor.receive(&mut ctx, any_message.as_view()).expect("save receive failed");

  let mut actor = SnapshotActor::<ScriptedSnapshotStore>::new_with_config(
    ScriptedSnapshotStore::new(1, 1, 1),
    SnapshotActorConfig::new(0),
  );
  let load = SnapshotMessage::LoadSnapshot {
    persistence_id: "pid-1".into(),
    criteria:       SnapshotSelectionCriteria::latest(),
    sender:         ActorRef::null(),
  };
  let any_message = AnyMessage::new(load);
  actor.receive(&mut ctx, any_message.as_view()).expect("load receive failed");

  let delete_one = SnapshotMessage::DeleteSnapshot { metadata, sender: ActorRef::null() };
  let any_message = AnyMessage::new(delete_one);
  actor.receive(&mut ctx, any_message.as_view()).expect("delete one receive failed");

  let delete_many = SnapshotMessage::DeleteSnapshots {
    persistence_id: "pid-1".into(),
    criteria:       SnapshotSelectionCriteria::latest(),
    sender:         ActorRef::null(),
  };
  let any_message = AnyMessage::new(delete_many);
  actor.receive(&mut ctx, any_message.as_view()).expect("delete many receive failed");
}
