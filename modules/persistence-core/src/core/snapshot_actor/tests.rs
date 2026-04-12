use core::future::{Pending, Ready, pending, ready};

use fraktor_actor_core_rs::core::kernel::{
  actor::{
    Actor, ActorCell, ActorContext, Pid,
    actor_ref::{ActorRef, ActorRefSender, ActorRefSenderShared, SendOutcome},
    error::{ActorError, SendError},
    messaging::{AnyMessage, AnyMessageView},
    props::Props,
  },
  system::{
    ActorSystem,
    state::{SystemStateShared, system_state::SystemState},
  },
};
use fraktor_utils_core_rs::core::sync::{ArcShared, SharedLock, SpinSyncMutex};

use super::SnapshotPoll;
use crate::core::{
  in_memory_snapshot_store::InMemorySnapshotStore, snapshot::Snapshot, snapshot_actor::SnapshotActor,
  snapshot_actor_config::SnapshotActorConfig, snapshot_error::SnapshotError, snapshot_message::SnapshotMessage,
  snapshot_metadata::SnapshotMetadata, snapshot_response::SnapshotResponse,
  snapshot_selection_criteria::SnapshotSelectionCriteria,
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

fn new_test_system() -> ActorSystem {
  let state = SystemState::new();
  let state = SystemStateShared::new(state);
  state.mark_root_started();
  register_actor_cell(&state, Pid::new(1, 1));
  ActorSystem::from_state(state)
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

impl crate::core::snapshot_store::SnapshotStore for PendingSnapshotStore {
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
    _snapshot: ArcShared<dyn core::any::Any + Send + Sync>,
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

impl crate::core::snapshot_store::SnapshotStore for RetrySnapshotStore {
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
    _snapshot: ArcShared<dyn core::any::Any + Send + Sync>,
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

#[test]
fn snapshot_actor_save_and_load_responses() {
  let system = new_test_system();
  let pid = Pid::new(1, 1);
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
  match response {
    | SnapshotResponse::LoadSnapshotResult { snapshot, to_sequence_nr } => {
      assert!(snapshot.is_some());
      assert_eq!(*to_sequence_nr, u64::MAX);
    },
    | _ => panic!("unexpected response"),
  }
}

#[test]
fn snapshot_actor_pending_does_not_emit_failure() {
  let system = new_test_system();
  let pid = Pid::new(1, 1);
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
fn snapshot_actor_retry_max_exceeded_on_errors() {
  let system = new_test_system();
  let pid = Pid::new(1, 1);
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
  match response {
    | SnapshotResponse::SaveSnapshotFailure { error, .. } => {
      assert_eq!(error, &SnapshotError::SaveFailed("boom".into()));
    },
    | _ => panic!("unexpected response"),
  }
}
