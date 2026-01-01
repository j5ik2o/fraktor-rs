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
use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdToolbox, RuntimeToolbox, SyncMutexFamily, ToolboxMutex},
  sync::ArcShared,
};

use crate::core::{
  in_memory_snapshot_store::InMemorySnapshotStore, snapshot_actor::SnapshotActor,
  snapshot_actor_config::SnapshotActorConfig, snapshot_error::SnapshotError, snapshot_message::SnapshotMessage,
  snapshot_metadata::SnapshotMetadata, snapshot_response::SnapshotResponse,
  snapshot_selection_criteria::SnapshotSelectionCriteria,
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

struct PendingSnapshotStore;

impl crate::core::snapshot_store::SnapshotStore for PendingSnapshotStore {
  type DeleteManyFuture<'a>
    = core::future::Pending<Result<(), SnapshotError>>
  where
    Self: 'a;
  type DeleteOneFuture<'a>
    = core::future::Pending<Result<(), SnapshotError>>
  where
    Self: 'a;
  type LoadFuture<'a>
    = core::future::Pending<Result<Option<crate::core::snapshot::Snapshot>, SnapshotError>>
  where
    Self: 'a;
  type SaveFuture<'a>
    = core::future::Pending<Result<(), SnapshotError>>
  where
    Self: 'a;

  fn save_snapshot<'a>(
    &'a mut self,
    _metadata: SnapshotMetadata,
    _snapshot: ArcShared<dyn core::any::Any + Send + Sync>,
  ) -> Self::SaveFuture<'a> {
    core::future::pending()
  }

  fn load_snapshot<'a>(
    &'a self,
    _persistence_id: &'a str,
    _criteria: SnapshotSelectionCriteria,
  ) -> Self::LoadFuture<'a> {
    core::future::pending()
  }

  fn delete_snapshot<'a>(&'a mut self, _metadata: &'a SnapshotMetadata) -> Self::DeleteOneFuture<'a> {
    core::future::pending()
  }

  fn delete_snapshots<'a>(
    &'a mut self,
    _persistence_id: &'a str,
    _criteria: SnapshotSelectionCriteria,
  ) -> Self::DeleteManyFuture<'a> {
    core::future::pending()
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
    = core::future::Ready<Result<(), SnapshotError>>
  where
    Self: 'a;
  type DeleteOneFuture<'a>
    = core::future::Ready<Result<(), SnapshotError>>
  where
    Self: 'a;
  type LoadFuture<'a>
    = core::future::Ready<Result<Option<crate::core::snapshot::Snapshot>, SnapshotError>>
  where
    Self: 'a;
  type SaveFuture<'a>
    = core::future::Ready<Result<(), SnapshotError>>
  where
    Self: 'a;

  fn save_snapshot<'a>(
    &'a mut self,
    _metadata: SnapshotMetadata,
    _snapshot: ArcShared<dyn core::any::Any + Send + Sync>,
  ) -> Self::SaveFuture<'a> {
    if self.failures_left > 0 {
      self.failures_left -= 1;
      core::future::ready(Err(SnapshotError::SaveFailed("boom".into())))
    } else {
      core::future::ready(Ok(()))
    }
  }

  fn load_snapshot<'a>(
    &'a self,
    _persistence_id: &'a str,
    _criteria: SnapshotSelectionCriteria,
  ) -> Self::LoadFuture<'a> {
    core::future::ready(Ok(None))
  }

  fn delete_snapshot<'a>(&'a mut self, _metadata: &'a SnapshotMetadata) -> Self::DeleteOneFuture<'a> {
    core::future::ready(Ok(()))
  }

  fn delete_snapshots<'a>(
    &'a mut self,
    _persistence_id: &'a str,
    _criteria: SnapshotSelectionCriteria,
  ) -> Self::DeleteManyFuture<'a> {
    core::future::ready(Ok(()))
  }
}

#[test]
fn snapshot_actor_save_and_load_responses() {
  let system = new_test_system();
  let pid = Pid::new(1, 1);
  let mut ctx = ActorContextGeneric::new(&system, pid);
  let mut actor = SnapshotActor::<InMemorySnapshotStore, TB>::new(InMemorySnapshotStore::new());
  let (sender, store) = create_sender();
  let metadata = SnapshotMetadata::new("pid-1", 1, 10);
  let payload = ArcShared::new(1_i32);

  let save = SnapshotMessage::SaveSnapshot { metadata: metadata.clone(), snapshot: payload, sender: sender.clone() };
  let any_message = AnyMessageGeneric::new(save);
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
  let any_message = AnyMessageGeneric::new(load);
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
  let mut ctx = ActorContextGeneric::new(&system, pid);
  let config = SnapshotActorConfig::new(0);
  let mut actor = SnapshotActor::<PendingSnapshotStore, TB>::new_with_config(PendingSnapshotStore, config);
  let (sender, store) = create_sender();

  let load = SnapshotMessage::LoadSnapshot {
    persistence_id: "pid-1".into(),
    criteria: SnapshotSelectionCriteria::latest(),
    sender,
  };
  let any_message = AnyMessageGeneric::new(load);
  actor.receive(&mut ctx, any_message.as_view()).expect("receive failed");

  let responses = store.lock();
  assert_eq!(responses.len(), 0);
}

#[test]
fn snapshot_actor_retry_max_exceeded_on_errors() {
  let system = new_test_system();
  let pid = Pid::new(1, 1);
  let mut ctx = ActorContextGeneric::new(&system, pid);
  let config = SnapshotActorConfig::new(1);
  let mut actor = SnapshotActor::<RetrySnapshotStore, TB>::new_with_config(RetrySnapshotStore::new(2), config);
  let (sender, store) = create_sender();
  let metadata = SnapshotMetadata::new("pid-1", 1, 10);
  let payload = ArcShared::new(1_i32);

  let save = SnapshotMessage::SaveSnapshot { metadata: metadata.clone(), snapshot: payload, sender };
  let any_message = AnyMessageGeneric::new(save);
  actor.receive(&mut ctx, any_message.as_view()).expect("receive failed");

  assert!(store.lock().is_empty());

  let poll = AnyMessageGeneric::new(super::SnapshotPoll);
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
