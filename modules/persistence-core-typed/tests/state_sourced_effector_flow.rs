#![cfg(not(target_os = "none"))]

use core::{
  future::{Future, ready},
  pin::Pin,
  task::{Context, Poll, Waker},
};
use std::{
  string::ToString,
  thread,
  time::{Duration, Instant},
  vec,
  vec::Vec,
};

use fraktor_actor_adaptor_std_rs::tick_driver::TestTickDriver;
use fraktor_actor_core_kernel_rs::{
  actor::{Pid, error::ActorError, setup::ActorSystemConfig},
  system::SpinBlocker,
};
use fraktor_actor_core_typed_rs::{
  Behavior, TypedActorRef, TypedActorSystem, TypedProps,
  actor::{TypedActor, TypedActorContext},
  dsl::{Behaviors, TypedAskError, TypedAskFuture},
};
use fraktor_persistence_core_kernel_rs::state::{
  DurableStateError, DurableStateStore, DurableStateStoreProvider, GetObjectResult,
};
use fraktor_persistence_core_typed_rs::{
  PersistenceId, StateSourcedEffector, StateSourcedEffectorConfig, StateSourcedEffectorMessageAdapter,
  StateSourcedEffectorSignal,
};
use fraktor_utils_core_rs::sync::{ArcShared, SpinSyncMutex};

type StoreFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T, DurableStateError>> + Send + 'a>>;
type SharedStoreRecord = ArcShared<SpinSyncMutex<StoreRecord>>;
type SharedValues<T> = ArcShared<SpinSyncMutex<Vec<T>>>;
type ManualResult<T> = ArcShared<SpinSyncMutex<PendingStoreResult<T>>>;

const PERSISTENCE_ID: &str = "typed-state-sourced-counter";

#[derive(Clone, Debug, PartialEq, Eq)]
struct CounterState {
  value: i32,
}

#[derive(Clone, Debug)]
enum CounterCommand {
  Set(i32),
  Delete,
  ReadState { reply_to: TypedActorRef<Option<CounterState>> },
  ReadRevision { reply_to: TypedActorRef<u64> },
  Persistence(StateSourcedEffectorSignal<CounterState>),
}

#[derive(Clone)]
enum CoordinatorCommand {
  Spawn { reply_to: TypedActorRef<TypedActorRef<CounterCommand>> },
}

#[derive(Clone)]
struct StoreRecord {
  state:    Option<CounterState>,
  revision: u64,
}

struct TestDurableStateStore {
  record: SharedStoreRecord,
}

impl TestDurableStateStore {
  fn new(record: SharedStoreRecord) -> Self {
    Self { record }
  }
}

impl DurableStateStore<CounterState> for TestDurableStateStore {
  fn get_object<'a>(&'a self, _persistence_id: &'a str) -> StoreFuture<'a, GetObjectResult<CounterState>> {
    let record = self.record.lock().clone();
    Box::pin(ready(Ok(GetObjectResult::new(record.state, record.revision))))
  }

  fn upsert_object<'a>(
    &'a mut self,
    persistence_id: &'a str,
    expected_revision: u64,
    object: CounterState,
    _tag: Option<&'a str>,
  ) -> StoreFuture<'a, ()> {
    let mut record = self.record.lock();
    if record.revision != expected_revision {
      return Box::pin(ready(Err(DurableStateError::upsert_revision(
        persistence_id,
        expected_revision,
        record.revision,
      ))));
    }
    record.state = Some(object);
    record.revision = record.revision.saturating_add(1);
    Box::pin(ready(Ok(())))
  }

  fn delete_object<'a>(&'a mut self, persistence_id: &'a str, expected_revision: u64) -> StoreFuture<'a, ()> {
    let mut record = self.record.lock();
    if record.revision != expected_revision {
      return Box::pin(ready(Err(DurableStateError::delete_revision(
        persistence_id,
        expected_revision,
        record.revision,
      ))));
    }
    record.state = None;
    record.revision = 0;
    Box::pin(ready(Ok(())))
  }
}

struct TestDurableStateStoreProvider {
  record: SharedStoreRecord,
}

impl TestDurableStateStoreProvider {
  fn new(record: SharedStoreRecord) -> Self {
    Self { record }
  }
}

impl DurableStateStoreProvider<CounterState> for TestDurableStateStoreProvider {
  fn durable_state_store(&self) -> Box<dyn DurableStateStore<CounterState>> {
    Box::new(TestDurableStateStore::new(self.record.clone()))
  }
}

struct PendingStoreResult<T> {
  result: Option<Result<T, DurableStateError>>,
  waker:  Option<Waker>,
}

struct ManualStoreFuture<T> {
  result: ManualResult<T>,
}

impl<T> ManualStoreFuture<T> {
  fn pending(result: ManualResult<T>) -> Self {
    Self { result }
  }
}

impl<T> Future for ManualStoreFuture<T> {
  type Output = Result<T, DurableStateError>;

  fn poll(self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<Self::Output> {
    let mut result = self.result.lock();
    match result.result.take() {
      | Some(value) => Poll::Ready(value),
      | None => {
        result.waker = Some(ctx.waker().clone());
        Poll::Pending
      },
    }
  }
}

#[derive(Clone)]
struct PendingUpsert {
  state:             CounterState,
  expected_revision: u64,
  result:            ManualResult<()>,
}

#[derive(Clone)]
struct PendingDelete {
  expected_revision: u64,
  result:            ManualResult<()>,
}

struct ManualStoreControl {
  recovery: Option<ManualResult<GetObjectResult<CounterState>>>,
  upserts:  Vec<PendingUpsert>,
  deletes:  Vec<PendingDelete>,
}

struct ManualDurableStateStore {
  control: ArcShared<SpinSyncMutex<ManualStoreControl>>,
}

impl ManualDurableStateStore {
  fn new(control: ArcShared<SpinSyncMutex<ManualStoreControl>>) -> Self {
    Self { control }
  }
}

impl DurableStateStore<CounterState> for ManualDurableStateStore {
  fn get_object<'a>(&'a self, _persistence_id: &'a str) -> StoreFuture<'a, GetObjectResult<CounterState>> {
    let result = pending_result();
    self.control.lock().recovery = Some(result.clone());
    Box::pin(ManualStoreFuture::pending(result))
  }

  fn upsert_object<'a>(
    &'a mut self,
    _persistence_id: &'a str,
    expected_revision: u64,
    object: CounterState,
    _tag: Option<&'a str>,
  ) -> StoreFuture<'a, ()> {
    let result = pending_result();
    self.control.lock().upserts.push(PendingUpsert { state: object, expected_revision, result: result.clone() });
    Box::pin(ManualStoreFuture::pending(result))
  }

  fn delete_object<'a>(&'a mut self, _persistence_id: &'a str, expected_revision: u64) -> StoreFuture<'a, ()> {
    let result = pending_result();
    self.control.lock().deletes.push(PendingDelete { expected_revision, result: result.clone() });
    Box::pin(ManualStoreFuture::pending(result))
  }
}

struct ManualDurableStateStoreProvider {
  control: ArcShared<SpinSyncMutex<ManualStoreControl>>,
}

impl ManualDurableStateStoreProvider {
  fn new(control: ArcShared<SpinSyncMutex<ManualStoreControl>>) -> Self {
    Self { control }
  }
}

impl DurableStateStoreProvider<CounterState> for ManualDurableStateStoreProvider {
  fn durable_state_store(&self) -> Box<dyn DurableStateStore<CounterState>> {
    Box::new(ManualDurableStateStore::new(self.control.clone()))
  }
}

#[derive(Clone)]
struct CounterProbe {
  ready_states:        SharedValues<Option<i32>>,
  ready_revisions:     SharedValues<u64>,
  persisted_revisions: SharedValues<u64>,
  deleted_revisions:   SharedValues<u64>,
  child_failures:      SharedValues<u64>,
}

impl CounterProbe {
  fn new() -> Self {
    Self {
      ready_states:        shared_values(),
      ready_revisions:     shared_values(),
      persisted_revisions: shared_values(),
      deleted_revisions:   shared_values(),
      child_failures:      shared_values(),
    }
  }
}

struct CounterCoordinator {
  config: StateSourcedEffectorConfig<CounterState, CounterCommand>,
  probe:  CounterProbe,
}

impl CounterCoordinator {
  fn new(config: StateSourcedEffectorConfig<CounterState, CounterCommand>, probe: CounterProbe) -> Self {
    Self { config, probe }
  }
}

impl TypedActor<CoordinatorCommand> for CounterCoordinator {
  fn receive(
    &mut self,
    ctx: &mut TypedActorContext<'_, CoordinatorCommand>,
    message: &CoordinatorCommand,
  ) -> Result<(), ActorError> {
    match message {
      | CoordinatorCommand::Spawn { reply_to } => {
        let props = counter_props(self.config.clone(), self.probe.clone());
        let child = ctx
          .spawn_child_watched(&props)
          .map_err(|error| ActorError::recoverable(format!("counter spawn failed: {error:?}")))?;
        let mut reply_to = reply_to.clone();
        reply_to.tell(child.actor_ref());
      },
    }
    Ok(())
  }

  fn on_child_failed(
    &mut self,
    _ctx: &mut TypedActorContext<'_, CoordinatorCommand>,
    child: Pid,
    _error: &ActorError,
  ) -> Result<(), ActorError> {
    self.probe.child_failures.lock().push(child.value());
    Ok(())
  }
}

fn counter_config(
  persistence_id: &str,
  provider: ArcShared<dyn DurableStateStoreProvider<CounterState>>,
) -> StateSourcedEffectorConfig<CounterState, CounterCommand> {
  let message_adapter = StateSourcedEffectorMessageAdapter::new(CounterCommand::Persistence, |message| match message {
    | CounterCommand::Persistence(signal) => Some(signal),
    | _ => None,
  });
  StateSourcedEffectorConfig::new(PersistenceId::of_unique_id(persistence_id.to_string()))
    .with_message_adapter(message_adapter)
    .with_store_provider(provider)
}

fn counter_props(
  config: StateSourcedEffectorConfig<CounterState, CounterCommand>,
  probe: CounterProbe,
) -> TypedProps<CounterCommand> {
  StateSourcedEffector::props(config, move |state, effector| {
    probe.ready_states.lock().push(state.as_ref().map(|state| state.value));
    probe.ready_revisions.lock().push(effector.revision());
    Ok(counter_behavior(state, effector, probe.clone()))
  })
}

fn counter_behavior(
  state: Option<CounterState>,
  effector: StateSourcedEffector<CounterState, CounterCommand>,
  probe: CounterProbe,
) -> Behavior<CounterCommand> {
  Behaviors::receive_message(move |_ctx, message| match message {
    | CounterCommand::Set(value) => {
      let next_state = CounterState { value: *value };
      let next_effector = effector.clone();
      let next_probe = probe.clone();
      effector.persist_state(next_state, move |persisted_state, revision| {
        next_probe.persisted_revisions.lock().push(revision);
        Ok(counter_behavior(Some(persisted_state.clone()), next_effector, next_probe))
      })
    },
    | CounterCommand::Delete => {
      let next_effector = effector.clone();
      let next_probe = probe.clone();
      effector.delete_state(move |revision| {
        next_probe.deleted_revisions.lock().push(revision);
        Ok(counter_behavior(None, next_effector, next_probe))
      })
    },
    | CounterCommand::ReadState { reply_to } => {
      let mut reply_to = reply_to.clone();
      reply_to.tell(state.clone());
      Ok(Behaviors::same())
    },
    | CounterCommand::ReadRevision { reply_to } => {
      let mut reply_to = reply_to.clone();
      reply_to.tell(effector.revision());
      Ok(Behaviors::same())
    },
    | CounterCommand::Persistence(_) => Ok(Behaviors::unhandled()),
  })
}

fn actor_system_config() -> ActorSystemConfig {
  ActorSystemConfig::new(TestTickDriver::default())
}

fn wait_until(deadline_ms: u64, mut predicate: impl FnMut() -> bool) -> bool {
  let deadline = Instant::now() + Duration::from_millis(deadline_ms);
  while Instant::now() < deadline {
    if predicate() {
      return true;
    }
    thread::yield_now();
  }
  predicate()
}

fn ask_state(actor: &mut TypedActorRef<CounterCommand>) -> Option<CounterState> {
  let mut future = start_ask_state(actor);
  assert!(wait_until(5000, || future.is_ready()));
  future.try_take().expect("state reply").expect("state payload")
}

fn ask_state_result(actor: &mut TypedActorRef<CounterCommand>) -> Result<Option<CounterState>, TypedAskError> {
  let mut future = start_ask_state(actor);
  assert!(wait_until(5000, || future.is_ready()));
  future.try_take().expect("state reply")
}

fn start_ask_state(actor: &mut TypedActorRef<CounterCommand>) -> TypedAskFuture<Option<CounterState>> {
  let response = actor.ask::<Option<CounterState>, _>(|reply_to| CounterCommand::ReadState { reply_to });
  response.future().clone()
}

fn ask_revision(actor: &mut TypedActorRef<CounterCommand>) -> u64 {
  let response = actor.ask::<u64, _>(|reply_to| CounterCommand::ReadRevision { reply_to });
  let mut future = response.future().clone();
  assert!(wait_until(5000, || future.is_ready()));
  future.try_take().expect("revision reply").expect("revision payload")
}

fn ask_spawn(coordinator: &mut TypedActorRef<CoordinatorCommand>) -> TypedActorRef<CounterCommand> {
  let response = coordinator.ask::<TypedActorRef<CounterCommand>, _>(|reply_to| CoordinatorCommand::Spawn { reply_to });
  let mut future = response.future().clone();
  assert!(wait_until(5000, || future.is_ready()));
  future.try_take().expect("spawn reply").expect("spawn payload")
}

fn terminate_system<M>(system: TypedActorSystem<M>)
where
  M: Send + Sync + 'static, {
  system.terminate().expect("terminate");
  system.as_untyped().run_until_terminated(&SpinBlocker);
}

fn shared_values<T>() -> SharedValues<T> {
  ArcShared::new(SpinSyncMutex::new(Vec::new()))
}

fn store_record(state: Option<CounterState>, revision: u64) -> SharedStoreRecord {
  ArcShared::new(SpinSyncMutex::new(StoreRecord { state, revision }))
}

fn store_provider(record: SharedStoreRecord) -> ArcShared<dyn DurableStateStoreProvider<CounterState>> {
  ArcShared::new(TestDurableStateStoreProvider::new(record))
}

fn pending_result<T>() -> ManualResult<T> {
  ArcShared::new(SpinSyncMutex::new(PendingStoreResult { result: None, waker: None }))
}

fn manual_store_control() -> ArcShared<SpinSyncMutex<ManualStoreControl>> {
  ArcShared::new(SpinSyncMutex::new(ManualStoreControl { recovery: None, upserts: Vec::new(), deletes: Vec::new() }))
}

fn manual_store_provider(
  control: ArcShared<SpinSyncMutex<ManualStoreControl>>,
) -> ArcShared<dyn DurableStateStoreProvider<CounterState>> {
  ArcShared::new(ManualDurableStateStoreProvider::new(control))
}

fn complete_manual_result<T>(result: &ManualResult<T>, value: Result<T, DurableStateError>) {
  let waker = {
    let mut result = result.lock();
    result.result = Some(value);
    result.waker.take()
  };
  if let Some(waker) = waker {
    waker.wake();
  }
}

fn wait_for_recovery(
  control: &ArcShared<SpinSyncMutex<ManualStoreControl>>,
) -> ManualResult<GetObjectResult<CounterState>> {
  assert!(wait_until(5000, || control.lock().recovery.is_some()));
  control.lock().recovery.as_ref().expect("recovery operation").clone()
}

fn wait_for_upsert(control: &ArcShared<SpinSyncMutex<ManualStoreControl>>, index: usize) -> PendingUpsert {
  assert!(wait_until(5000, || control.lock().upserts.len() > index));
  control.lock().upserts[index].clone()
}

fn wait_for_delete(control: &ArcShared<SpinSyncMutex<ManualStoreControl>>, index: usize) -> PendingDelete {
  assert!(wait_until(5000, || control.lock().deletes.len() > index));
  control.lock().deletes[index].clone()
}

#[test]
fn recovered_state_can_be_persisted_and_deleted() {
  let record = store_record(Some(CounterState { value: 10 }), 3);
  let probe = CounterProbe::new();
  let assertion_probe = probe.clone();
  let config = counter_config(PERSISTENCE_ID, store_provider(record.clone()));
  let coordinator_props = TypedProps::new(move || CounterCoordinator::new(config.clone(), probe.clone()));
  let system = TypedActorSystem::<CoordinatorCommand>::create_from_props(&coordinator_props, actor_system_config())
    .expect("system");
  let mut coordinator = system.user_guardian_ref();
  let mut actor = ask_spawn(&mut coordinator);

  assert_eq!(ask_state(&mut actor), Some(CounterState { value: 10 }));
  assert_eq!(ask_revision(&mut actor), 3);

  actor.tell(CounterCommand::Set(12));
  assert_eq!(ask_state(&mut actor), Some(CounterState { value: 12 }));
  assert_eq!(ask_revision(&mut actor), 4);

  actor.tell(CounterCommand::Delete);
  assert_eq!(ask_state(&mut actor), None);
  assert_eq!(ask_revision(&mut actor), 0);

  actor.tell(CounterCommand::Set(13));
  assert_eq!(ask_state(&mut actor), Some(CounterState { value: 13 }));
  assert_eq!(ask_revision(&mut actor), 1);

  assert_eq!(*assertion_probe.ready_states.lock(), vec![Some(10)]);
  assert_eq!(*assertion_probe.ready_revisions.lock(), vec![3]);
  assert_eq!(*assertion_probe.persisted_revisions.lock(), vec![4, 1]);
  assert_eq!(*assertion_probe.deleted_revisions.lock(), vec![0]);
  assert_eq!(record.lock().state, Some(CounterState { value: 13 }));
  assert_eq!(record.lock().revision, 1);

  terminate_system(system);
}

#[test]
fn present_recovery_starts_with_recovered_state_and_revision() {
  let record = store_record(Some(CounterState { value: 15 }), 2);
  let probe = CounterProbe::new();
  let config = counter_config("typed-state-sourced-present-counter", store_provider(record));
  let props = counter_props(config, probe.clone());
  let system = TypedActorSystem::<CounterCommand>::create_from_props(&props, actor_system_config()).expect("system");
  let mut actor = system.user_guardian_ref();

  assert_eq!(ask_state(&mut actor), Some(CounterState { value: 15 }));
  assert_eq!(ask_revision(&mut actor), 2);
  assert_eq!(*probe.ready_states.lock(), vec![Some(15)]);
  assert_eq!(*probe.ready_revisions.lock(), vec![2]);

  terminate_system(system);
}

#[test]
fn empty_recovery_starts_empty_behavior_with_zero_revision() {
  let record = store_record(None, 0);
  let probe = CounterProbe::new();
  let config = counter_config("typed-state-sourced-empty-counter", store_provider(record));
  let props = counter_props(config, probe.clone());
  let system = TypedActorSystem::<CounterCommand>::create_from_props(&props, actor_system_config()).expect("system");
  let mut actor = system.user_guardian_ref();

  assert_eq!(ask_state(&mut actor), None);
  assert_eq!(ask_revision(&mut actor), 0);
  assert_eq!(*probe.ready_states.lock(), vec![None]);
  assert_eq!(*probe.ready_revisions.lock(), vec![0]);

  terminate_system(system);
}

#[test]
fn user_command_sent_during_recovery_is_stashed_until_recovery_completes() {
  let control = manual_store_control();
  let probe = CounterProbe::new();
  let config = counter_config("typed-state-sourced-recovery-stash-counter", manual_store_provider(control.clone()));
  let props = counter_props(config, probe.clone());
  let system = TypedActorSystem::<CounterCommand>::create_from_props(&props, actor_system_config()).expect("system");
  let mut actor = system.user_guardian_ref();
  let recovery = wait_for_recovery(&control);

  actor.tell(CounterCommand::Set(21));
  assert!(control.lock().upserts.is_empty());
  assert!(probe.ready_states.lock().is_empty());

  complete_manual_result(&recovery, Ok(GetObjectResult::new(Some(CounterState { value: 10 }), 3)));
  let upsert = wait_for_upsert(&control, 0);
  assert_eq!(upsert.expected_revision, 3);
  assert_eq!(upsert.state, CounterState { value: 21 });

  complete_manual_result(&upsert.result, Ok(()));

  assert_eq!(ask_state(&mut actor), Some(CounterState { value: 21 }));
  assert_eq!(ask_revision(&mut actor), 4);
  assert_eq!(*probe.ready_states.lock(), vec![Some(10)]);
  assert_eq!(*probe.ready_revisions.lock(), vec![3]);
  assert_eq!(*probe.persisted_revisions.lock(), vec![4]);

  terminate_system(system);
}

#[test]
fn user_command_sent_during_persist_is_stashed_until_persist_completes() {
  let control = manual_store_control();
  let probe = CounterProbe::new();
  let config = counter_config("typed-state-sourced-persist-stash-counter", manual_store_provider(control.clone()));
  let props = counter_props(config, probe.clone());
  let system = TypedActorSystem::<CounterCommand>::create_from_props(&props, actor_system_config()).expect("system");
  let mut actor = system.user_guardian_ref();
  let recovery = wait_for_recovery(&control);
  complete_manual_result(&recovery, Ok(GetObjectResult::new(Some(CounterState { value: 10 }), 3)));
  assert_eq!(ask_state(&mut actor), Some(CounterState { value: 10 }));

  actor.tell(CounterCommand::Set(30));
  let upsert = wait_for_upsert(&control, 0);
  let mut pending_state = start_ask_state(&mut actor);

  assert!(!pending_state.is_ready());

  complete_manual_result(&upsert.result, Ok(()));

  assert!(wait_until(5000, || pending_state.is_ready()));
  assert_eq!(pending_state.try_take().expect("state reply").expect("state payload"), Some(CounterState { value: 30 }));
  assert_eq!(*probe.persisted_revisions.lock(), vec![4]);

  terminate_system(system);
}

#[test]
fn user_command_sent_during_delete_is_stashed_until_delete_completes() {
  let control = manual_store_control();
  let probe = CounterProbe::new();
  let config = counter_config("typed-state-sourced-delete-stash-counter", manual_store_provider(control.clone()));
  let props = counter_props(config, probe.clone());
  let system = TypedActorSystem::<CounterCommand>::create_from_props(&props, actor_system_config()).expect("system");
  let mut actor = system.user_guardian_ref();
  let recovery = wait_for_recovery(&control);
  complete_manual_result(&recovery, Ok(GetObjectResult::new(Some(CounterState { value: 10 }), 3)));
  assert_eq!(ask_state(&mut actor), Some(CounterState { value: 10 }));

  actor.tell(CounterCommand::Delete);
  let delete = wait_for_delete(&control, 0);
  let mut pending_state = start_ask_state(&mut actor);

  assert!(!pending_state.is_ready());
  assert_eq!(delete.expected_revision, 3);

  complete_manual_result(&delete.result, Ok(()));

  assert!(wait_until(5000, || pending_state.is_ready()));
  assert_eq!(pending_state.try_take().expect("state reply").expect("state payload"), None);
  assert_eq!(*probe.deleted_revisions.lock(), vec![0]);

  terminate_system(system);
}

#[test]
fn recovery_store_failure_stops_child_without_ready_callback() {
  let control = manual_store_control();
  let probe = CounterProbe::new();
  let config = counter_config("typed-state-sourced-recovery-failure-counter", manual_store_provider(control.clone()));
  let coordinator_props = TypedProps::new({
    let probe = probe.clone();
    move || CounterCoordinator::new(config.clone(), probe.clone())
  });
  let system = TypedActorSystem::<CoordinatorCommand>::create_from_props(&coordinator_props, actor_system_config())
    .expect("system");
  let mut coordinator = system.user_guardian_ref();
  let mut actor = ask_spawn(&mut coordinator);
  let recovery = wait_for_recovery(&control);

  complete_manual_result(&recovery, Err(DurableStateError::GetObjectFailed("store unavailable".to_string())));

  assert!(wait_until(5000, || !probe.child_failures.lock().is_empty()));
  assert_eq!(*probe.ready_states.lock(), Vec::<Option<i32>>::new());
  assert_eq!(*probe.ready_revisions.lock(), Vec::<u64>::new());
  assert!(matches!(ask_state_result(&mut actor), Err(TypedAskError::AskFailed(_))));

  terminate_system(system);
}

#[test]
fn persist_store_failure_stops_child_without_running_success_callback() {
  let control = manual_store_control();
  let probe = CounterProbe::new();
  let config = counter_config("typed-state-sourced-persist-failure-counter", manual_store_provider(control.clone()));
  let coordinator_props = TypedProps::new({
    let probe = probe.clone();
    move || CounterCoordinator::new(config.clone(), probe.clone())
  });
  let system = TypedActorSystem::<CoordinatorCommand>::create_from_props(&coordinator_props, actor_system_config())
    .expect("system");
  let mut coordinator = system.user_guardian_ref();
  let mut actor = ask_spawn(&mut coordinator);
  let recovery = wait_for_recovery(&control);
  complete_manual_result(&recovery, Ok(GetObjectResult::new(Some(CounterState { value: 10 }), 3)));
  assert_eq!(ask_state(&mut actor), Some(CounterState { value: 10 }));

  actor.tell(CounterCommand::Set(40));
  let upsert = wait_for_upsert(&control, 0);
  assert_eq!(upsert.expected_revision, 3);
  assert_eq!(upsert.state, CounterState { value: 40 });

  complete_manual_result(&upsert.result, Err(DurableStateError::UpsertObjectFailed("write failed".to_string())));

  assert!(wait_until(5000, || !probe.child_failures.lock().is_empty()));
  assert_eq!(control.lock().upserts.len(), 1);
  assert_eq!(*probe.persisted_revisions.lock(), Vec::<u64>::new());
  assert!(matches!(ask_state_result(&mut actor), Err(TypedAskError::AskFailed(_))));

  terminate_system(system);
}

#[test]
fn delete_store_failure_stops_child_without_running_success_callback() {
  let control = manual_store_control();
  let probe = CounterProbe::new();
  let config = counter_config("typed-state-sourced-delete-failure-counter", manual_store_provider(control.clone()));
  let coordinator_props = TypedProps::new({
    let probe = probe.clone();
    move || CounterCoordinator::new(config.clone(), probe.clone())
  });
  let system = TypedActorSystem::<CoordinatorCommand>::create_from_props(&coordinator_props, actor_system_config())
    .expect("system");
  let mut coordinator = system.user_guardian_ref();
  let mut actor = ask_spawn(&mut coordinator);
  let recovery = wait_for_recovery(&control);
  complete_manual_result(&recovery, Ok(GetObjectResult::new(Some(CounterState { value: 10 }), 3)));
  assert_eq!(ask_state(&mut actor), Some(CounterState { value: 10 }));

  actor.tell(CounterCommand::Delete);
  let delete = wait_for_delete(&control, 0);
  assert_eq!(delete.expected_revision, 3);

  complete_manual_result(&delete.result, Err(DurableStateError::DeleteObjectFailed("delete failed".to_string())));

  assert!(wait_until(5000, || !probe.child_failures.lock().is_empty()));
  assert_eq!(control.lock().deletes.len(), 1);
  assert_eq!(*probe.deleted_revisions.lock(), Vec::<u64>::new());
  assert!(matches!(ask_state_result(&mut actor), Err(TypedAskError::AskFailed(_))));

  terminate_system(system);
}

#[test]
fn max_revision_persist_flow_keeps_saturated_revision() {
  let record = store_record(Some(CounterState { value: 10 }), u64::MAX);
  let probe = CounterProbe::new();
  let config = counter_config("typed-state-sourced-max-revision-counter", store_provider(record.clone()));
  let props = counter_props(config, probe.clone());
  let system = TypedActorSystem::<CounterCommand>::create_from_props(&props, actor_system_config()).expect("system");
  let mut actor = system.user_guardian_ref();

  assert_eq!(ask_state(&mut actor), Some(CounterState { value: 10 }));
  assert_eq!(ask_revision(&mut actor), u64::MAX);

  actor.tell(CounterCommand::Set(50));

  assert_eq!(ask_state(&mut actor), Some(CounterState { value: 50 }));
  assert_eq!(ask_revision(&mut actor), u64::MAX);
  assert_eq!(*probe.persisted_revisions.lock(), vec![u64::MAX]);
  assert_eq!(record.lock().state, Some(CounterState { value: 50 }));
  assert_eq!(record.lock().revision, u64::MAX);

  terminate_system(system);
}

#[test]
fn revision_mismatch_failure_stops_child_without_running_success_callback() {
  let record = store_record(Some(CounterState { value: 10 }), 3);
  let probe = CounterProbe::new();
  let config = counter_config("typed-state-sourced-revision-mismatch-counter", store_provider(record.clone()));
  let coordinator_props = TypedProps::new({
    let probe = probe.clone();
    move || CounterCoordinator::new(config.clone(), probe.clone())
  });
  let system = TypedActorSystem::<CoordinatorCommand>::create_from_props(&coordinator_props, actor_system_config())
    .expect("system");
  let mut coordinator = system.user_guardian_ref();
  let mut actor = ask_spawn(&mut coordinator);

  assert_eq!(ask_state(&mut actor), Some(CounterState { value: 10 }));
  record.lock().revision = 4;

  actor.tell(CounterCommand::Set(40));

  assert!(wait_until(5000, || !probe.child_failures.lock().is_empty()));
  assert_eq!(*probe.persisted_revisions.lock(), Vec::<u64>::new());
  assert_eq!(record.lock().state, Some(CounterState { value: 10 }));
  assert_eq!(record.lock().revision, 4);
  assert!(matches!(ask_state_result(&mut actor), Err(TypedAskError::AskFailed(_))));

  terminate_system(system);
}

#[test]
fn delete_revision_mismatch_failure_stops_child_without_running_success_callback() {
  let record = store_record(Some(CounterState { value: 10 }), 4);
  let probe = CounterProbe::new();
  let config = counter_config("typed-state-sourced-delete-revision-mismatch-counter", store_provider(record.clone()));
  let coordinator_props = TypedProps::new({
    let probe = probe.clone();
    move || CounterCoordinator::new(config.clone(), probe.clone())
  });
  let system = TypedActorSystem::<CoordinatorCommand>::create_from_props(&coordinator_props, actor_system_config())
    .expect("system");
  let mut coordinator = system.user_guardian_ref();
  let mut actor = ask_spawn(&mut coordinator);

  assert_eq!(ask_state(&mut actor), Some(CounterState { value: 10 }));
  assert_eq!(ask_revision(&mut actor), 4);
  record.lock().revision = 5;

  actor.tell(CounterCommand::Delete);

  assert!(wait_until(5000, || !probe.child_failures.lock().is_empty()));
  assert_eq!(*probe.deleted_revisions.lock(), Vec::<u64>::new());
  assert_eq!(record.lock().state, Some(CounterState { value: 10 }));
  assert_eq!(record.lock().revision, 5);
  assert!(matches!(ask_state_result(&mut actor), Err(TypedAskError::AskFailed(_))));

  terminate_system(system);
}
