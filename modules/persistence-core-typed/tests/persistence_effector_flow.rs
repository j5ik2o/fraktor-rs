#![cfg(not(target_os = "none"))]

use core::{
  any::Any,
  future::Future,
  task::{Context, Poll, Waker},
};
use std::{
  format,
  string::{String, ToString},
  thread,
  time::{Duration, Instant},
  vec,
  vec::Vec,
};

use fraktor_actor_adaptor_std_rs::tick_driver::TestTickDriver;
use fraktor_actor_core_kernel_rs::{
  actor::{error::ActorError, extension::ExtensionInstallers, scheduler::SchedulerConfig, setup::ActorSystemConfig},
  system::SpinBlocker,
};
use fraktor_actor_core_typed_rs::{
  Behavior, TypedActorRef, TypedActorSystem, TypedProps,
  actor::{TypedActor, TypedActorContext, TypedChildRef},
  dsl::Behaviors,
};
use fraktor_persistence_core_kernel_rs::{
  extension::PersistenceExtensionInstaller,
  journal::{InMemoryJournal, Journal},
  persistent::PersistentRepr,
  snapshot::{InMemorySnapshotStore, SnapshotMetadata, SnapshotStore},
};
use fraktor_persistence_core_typed_rs::{
  PersistenceEffector, PersistenceEffectorConfig, PersistenceEffectorMessageAdapter, PersistenceEffectorSignal,
  PersistenceId, PersistenceMode, RetentionCriteria, SnapshotCriteria,
};
use fraktor_utils_core_rs::sync::{ArcShared, SpinSyncMutex};

type SharedValues = ArcShared<SpinSyncMutex<Vec<i32>>>;

#[derive(Clone, Debug, PartialEq, Eq)]
struct CounterState {
  value: i32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum CounterEvent {
  Added(i32),
}

#[derive(Clone, Debug)]
enum CounterCommand {
  Add(i32),
  AddThenRecovery(i32),
  AddBatch(Vec<i32>),
  AddWithSnapshot(i32),
  Reject { reply_to: TypedActorRef<String> },
  ReadValue { reply_to: TypedActorRef<i32> },
  ReadSequence { reply_to: TypedActorRef<u64> },
  Persistence(PersistenceEffectorSignal<CounterState, CounterEvent>),
}

#[derive(Clone)]
enum ManagerCommand {
  Spawn { reply_to: TypedActorRef<TypedActorRef<CounterCommand>> },
  Stop,
}

struct CounterManager {
  config:       PersistenceEffectorConfig<CounterState, CounterEvent, CounterCommand>,
  command_log:  SharedValues,
  ready_values: SharedValues,
  child:        Option<TypedChildRef<CounterCommand>>,
}

impl CounterManager {
  fn new(
    config: PersistenceEffectorConfig<CounterState, CounterEvent, CounterCommand>,
    command_log: SharedValues,
    ready_values: SharedValues,
  ) -> Self {
    Self { config, command_log, ready_values, child: None }
  }
}

impl TypedActor<ManagerCommand> for CounterManager {
  fn receive(
    &mut self,
    ctx: &mut TypedActorContext<'_, ManagerCommand>,
    message: &ManagerCommand,
  ) -> Result<(), ActorError> {
    match message {
      | ManagerCommand::Spawn { reply_to } => {
        let props = counter_props(self.config.clone(), self.command_log.clone(), self.ready_values.clone());
        let child = ctx
          .spawn_child(&props)
          .map_err(|error| ActorError::recoverable(format!("counter spawn failed: {error:?}")))?;
        let mut reply_to = reply_to.clone();
        reply_to.tell(child.actor_ref());
        self.child = Some(child);
      },
      | ManagerCommand::Stop => {
        if let Some(child) = self.child.take() {
          child.stop().map_err(|error| ActorError::recoverable(format!("counter stop failed: {error:?}")))?;
        }
      },
    }
    Ok(())
  }
}

fn apply_counter_event(state: &CounterState, event: &CounterEvent) -> CounterState {
  let CounterEvent::Added(delta) = event;
  CounterState { value: state.value + delta }
}

fn counter_config(
  mode: PersistenceMode,
  persistence_id: &str,
) -> PersistenceEffectorConfig<CounterState, CounterEvent, CounterCommand> {
  let message_adapter = PersistenceEffectorMessageAdapter::new(CounterCommand::Persistence, |message| match message {
    | CounterCommand::Persistence(signal) => Some(signal),
    | _ => None,
  });
  PersistenceEffectorConfig::new(
    PersistenceId::of_unique_id(persistence_id.to_string()),
    CounterState { value: 0 },
    apply_counter_event,
  )
  .with_persistence_mode(mode)
  .with_message_adapter(message_adapter)
}

fn counter_props(
  config: PersistenceEffectorConfig<CounterState, CounterEvent, CounterCommand>,
  command_log: SharedValues,
  ready_values: SharedValues,
) -> TypedProps<CounterCommand> {
  PersistenceEffector::props(config, move |state, effector| {
    ready_values.lock().push(state.value);
    Ok(counter_behavior(state, effector, command_log.clone()))
  })
}

fn counter_behavior(
  state: CounterState,
  effector: PersistenceEffector<CounterState, CounterEvent, CounterCommand>,
  command_log: SharedValues,
) -> Behavior<CounterCommand> {
  Behaviors::receive_message(move |ctx, message| match message {
    | CounterCommand::Add(delta) => {
      command_log.lock().push(*delta);
      let event = CounterEvent::Added(*delta);
      let next_state = apply_counter_event(&state, &event);
      let next_effector = effector.clone();
      let next_command_log = command_log.clone();
      effector
        .persist_event(ctx, event, move |_event| Ok(counter_behavior(next_state, next_effector, next_command_log)))
    },
    | CounterCommand::AddThenRecovery(delta) => {
      command_log.lock().push(*delta);
      let event = CounterEvent::Added(*delta);
      let next_state = apply_counter_event(&state, &event);
      let next_sequence_nr = effector.sequence_nr().saturating_add(1);
      let mut self_ref = ctx.self_ref();
      self_ref
        .try_tell(CounterCommand::Persistence(PersistenceEffectorSignal::RecoveryCompleted {
          state:       next_state.clone(),
          sequence_nr: next_sequence_nr,
        }))
        .map_err(|error| ActorError::fatal(format!("recovery completion self-send failed: {error:?}")))?;
      let next_effector = effector.clone();
      let next_command_log = command_log.clone();
      effector
        .persist_event(ctx, event, move |_event| Ok(counter_behavior(next_state, next_effector, next_command_log)))
    },
    | CounterCommand::AddBatch(deltas) => {
      for delta in deltas.iter() {
        command_log.lock().push(*delta);
      }
      let events = deltas.iter().copied().map(CounterEvent::Added).collect::<Vec<_>>();
      let next_state = events.iter().fold(state.clone(), |state, event| apply_counter_event(&state, event));
      let next_effector = effector.clone();
      let next_command_log = command_log.clone();
      effector
        .persist_events(ctx, events, move |_events| Ok(counter_behavior(next_state, next_effector, next_command_log)))
    },
    | CounterCommand::AddWithSnapshot(delta) => {
      command_log.lock().push(*delta);
      let event = CounterEvent::Added(*delta);
      let next_state = apply_counter_event(&state, &event);
      let snapshot = next_state.clone();
      let next_effector = effector.clone();
      let next_command_log = command_log.clone();
      effector.persist_event_with_snapshot(ctx, event, snapshot, false, move |_event| {
        Ok(counter_behavior(next_state, next_effector, next_command_log))
      })
    },
    | CounterCommand::Reject { reply_to } => {
      let mut reply_to = reply_to.clone();
      reply_to.tell(String::from("domain-error"));
      Ok(Behaviors::same())
    },
    | CounterCommand::ReadValue { reply_to } => {
      let mut reply_to = reply_to.clone();
      reply_to.tell(state.value);
      Ok(Behaviors::same())
    },
    | CounterCommand::ReadSequence { reply_to } => {
      let mut reply_to = reply_to.clone();
      reply_to.tell(effector.sequence_nr());
      Ok(Behaviors::same())
    },
    | CounterCommand::Persistence(_) => Ok(Behaviors::unhandled()),
  })
}

fn actor_system_config() -> ActorSystemConfig {
  ActorSystemConfig::new(TestTickDriver::default())
}

fn actor_system_config_with_persistence(
  journal: InMemoryJournal,
  snapshot_store: InMemorySnapshotStore,
) -> ActorSystemConfig {
  let installer = PersistenceExtensionInstaller::new(journal, snapshot_store);
  let installers = ExtensionInstallers::default().with_extension_installer(installer);
  let scheduler = SchedulerConfig::default().with_runner_api_enabled(true);
  ActorSystemConfig::new(TestTickDriver::default())
    .with_scheduler_config(scheduler)
    .with_extension_installers(installers)
}

fn drive_ready<F: Future>(future: F) -> F::Output {
  let waker = Waker::noop();
  let mut context = Context::from_waker(waker);
  let mut future = core::pin::pin!(future);
  match Future::poll(future.as_mut(), &mut context) {
    | Poll::Ready(output) => output,
    | Poll::Pending => panic!("future was not ready"),
  }
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

fn ask_value(actor: &mut TypedActorRef<CounterCommand>) -> i32 {
  let response = actor.ask::<i32, _>(|reply_to| CounterCommand::ReadValue { reply_to });
  let mut future = response.future().clone();
  assert!(wait_until(5000, || future.is_ready()));
  future.try_take().expect("value reply").expect("value payload")
}

fn ask_sequence(actor: &mut TypedActorRef<CounterCommand>) -> u64 {
  let response = actor.ask::<u64, _>(|reply_to| CounterCommand::ReadSequence { reply_to });
  let mut future = response.future().clone();
  assert!(wait_until(5000, || future.is_ready()));
  future.try_take().expect("sequence reply").expect("sequence payload")
}

fn ask_rejection(actor: &mut TypedActorRef<CounterCommand>) -> String {
  let response = actor.ask::<String, _>(|reply_to| CounterCommand::Reject { reply_to });
  let mut future = response.future().clone();
  assert!(wait_until(5000, || future.is_ready()));
  future.try_take().expect("rejection reply").expect("rejection payload")
}

fn ask_spawn(manager: &mut TypedActorRef<ManagerCommand>) -> TypedActorRef<CounterCommand> {
  let response = manager.ask::<TypedActorRef<CounterCommand>, _>(|reply_to| ManagerCommand::Spawn { reply_to });
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

#[test]
fn persisted_mode_recovers_snapshot_replays_events_and_persists_new_events() {
  const PERSISTENCE_ID: &str = "typed-persisted-counter";

  let mut journal = InMemoryJournal::new();
  let repr0 = PersistentRepr::new(PERSISTENCE_ID, 1, ArcShared::new(CounterEvent::Added(4)));
  let repr1 = PersistentRepr::new(PERSISTENCE_ID, 2, ArcShared::new(CounterEvent::Added(6)));
  let repr2 = PersistentRepr::new(PERSISTENCE_ID, 3, ArcShared::new(CounterEvent::Added(2)));
  drive_ready(journal.write_messages(&[repr0, repr1, repr2])).expect("seed journal");

  let mut snapshot_store = InMemorySnapshotStore::new();
  let snapshot_metadata = SnapshotMetadata::new(PERSISTENCE_ID, 2, 0);
  let snapshot_payload: ArcShared<dyn Any + Send + Sync> = ArcShared::new(CounterState { value: 10 });
  drive_ready(snapshot_store.save_snapshot(snapshot_metadata, snapshot_payload)).expect("seed snapshot");

  let command_log = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let ready_values = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let config = counter_config(PersistenceMode::Persisted, PERSISTENCE_ID);
  let props = counter_props(config, command_log.clone(), ready_values.clone());
  let system = TypedActorSystem::<CounterCommand>::create_from_props(
    &props,
    actor_system_config_with_persistence(journal, snapshot_store),
  )
  .expect("system");
  let mut actor = system.user_guardian_ref();

  actor.tell(CounterCommand::Add(5));
  assert_eq!(ask_value(&mut actor), 17);

  actor.tell(CounterCommand::AddBatch(vec![1, 2, 3]));
  assert_eq!(ask_value(&mut actor), 23);
  assert_eq!(ask_sequence(&mut actor), 7);
  assert_eq!(*ready_values.lock(), vec![12]);
  assert_eq!(*command_log.lock(), vec![5, 1, 2, 3]);

  terminate_system(system);
}

#[test]
fn persisted_mode_runs_recovery_and_on_ready_during_startup() {
  const PERSISTENCE_ID: &str = "typed-persisted-eager-start-counter";

  let mut journal = InMemoryJournal::new();
  let repr = PersistentRepr::new(PERSISTENCE_ID, 1, ArcShared::new(CounterEvent::Added(3)));
  drive_ready(journal.write_messages(&[repr])).expect("seed journal");

  let snapshot_store = InMemorySnapshotStore::new();
  let command_log = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let ready_values = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let props = counter_props(
    counter_config(PersistenceMode::Persisted, PERSISTENCE_ID),
    command_log.clone(),
    ready_values.clone(),
  );
  let system = TypedActorSystem::<CounterCommand>::create_from_props(
    &props,
    actor_system_config_with_persistence(journal, snapshot_store),
  )
  .expect("system");
  let mut actor = system.user_guardian_ref();

  assert!(wait_until(5000, || *ready_values.lock() == vec![3]));
  assert_eq!(ask_value(&mut actor), 3);
  assert_eq!(*command_log.lock(), Vec::<i32>::new());

  terminate_system(system);
}

#[test]
fn persisted_mode_resynchronizes_when_recovery_completes_during_persist_wait() {
  const PERSISTENCE_ID: &str = "typed-persisted-restart-resync-counter";

  let journal = InMemoryJournal::new();
  let snapshot_store = InMemorySnapshotStore::new();
  let command_log = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let ready_values = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let props = counter_props(
    counter_config(PersistenceMode::Persisted, PERSISTENCE_ID),
    command_log.clone(),
    ready_values.clone(),
  );
  let system = TypedActorSystem::<CounterCommand>::create_from_props(
    &props,
    actor_system_config_with_persistence(journal, snapshot_store),
  )
  .expect("system");
  let mut actor = system.user_guardian_ref();

  assert_eq!(ask_value(&mut actor), 0);
  actor.tell(CounterCommand::AddThenRecovery(1));

  assert!(wait_until(5000, || *ready_values.lock() == vec![0, 1]));
  assert_eq!(ask_value(&mut actor), 1);
  assert_eq!(ask_sequence(&mut actor), 1);
  assert_eq!(*command_log.lock(), vec![1]);

  terminate_system(system);
}

#[test]
fn persisted_mode_unstashes_commands_when_event_persist_triggers_snapshot() {
  const PERSISTENCE_ID: &str = "typed-persisted-snapshot-stash-counter";

  let journal = InMemoryJournal::new();
  let snapshot_store = InMemorySnapshotStore::new();
  let command_log = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let ready_values = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let config =
    counter_config(PersistenceMode::Persisted, PERSISTENCE_ID).with_snapshot_criteria(SnapshotCriteria::always());
  let props = counter_props(config, command_log.clone(), ready_values.clone());
  let system = TypedActorSystem::<CounterCommand>::create_from_props(
    &props,
    actor_system_config_with_persistence(journal, snapshot_store),
  )
  .expect("system");
  let mut actor = system.user_guardian_ref();

  actor.tell(CounterCommand::AddWithSnapshot(1));
  actor.tell(CounterCommand::Add(2));

  assert_eq!(ask_value(&mut actor), 3);
  assert_eq!(ask_sequence(&mut actor), 2);
  assert_eq!(*ready_values.lock(), vec![0]);
  assert_eq!(*command_log.lock(), vec![1, 2]);

  terminate_system(system);
}

#[test]
fn ephemeral_mode_replays_from_actor_system_scoped_store() {
  const PERSISTENCE_ID: &str = "typed-ephemeral-counter";

  let command_log = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let ready_values = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let config = counter_config(PersistenceMode::Ephemeral, PERSISTENCE_ID)
    .with_snapshot_criteria(SnapshotCriteria::every(2))
    .with_retention_criteria(RetentionCriteria::snapshot_every(2, 2));
  let props = TypedProps::<ManagerCommand>::new({
    let command_log = command_log.clone();
    let ready_values = ready_values.clone();
    move || CounterManager::new(config.clone(), command_log.clone(), ready_values.clone())
  });
  let system = TypedActorSystem::<ManagerCommand>::create_from_props(&props, actor_system_config()).expect("system");
  let mut manager = system.user_guardian_ref();

  let mut first = ask_spawn(&mut manager);
  first.tell(CounterCommand::Add(3));
  first.tell(CounterCommand::AddWithSnapshot(4));
  assert_eq!(ask_value(&mut first), 7);
  assert_eq!(ask_sequence(&mut first), 2);

  manager.tell(ManagerCommand::Stop);
  let mut second = ask_spawn(&mut manager);
  assert_eq!(ask_value(&mut second), 7);
  assert_eq!(ask_sequence(&mut second), 2);
  assert_eq!(*ready_values.lock(), vec![0, 7]);
  assert_eq!(*command_log.lock(), vec![3, 4]);

  terminate_system(system);
}

#[test]
fn deferred_mode_runs_callbacks_without_recovery_storage() {
  const PERSISTENCE_ID: &str = "typed-deferred-counter";

  let command_log = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let ready_values = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let props =
    counter_props(counter_config(PersistenceMode::Deferred, PERSISTENCE_ID), command_log.clone(), ready_values.clone());
  let system = TypedActorSystem::<CounterCommand>::create_from_props(&props, actor_system_config()).expect("system");
  let mut actor = system.user_guardian_ref();

  actor.tell(CounterCommand::Add(9));
  assert_eq!(ask_value(&mut actor), 9);
  assert_eq!(ask_sequence(&mut actor), 0);
  assert_eq!(*ready_values.lock(), vec![0]);
  assert_eq!(*command_log.lock(), vec![9]);
  terminate_system(system);

  let second_ready_values = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let second_props = counter_props(
    counter_config(PersistenceMode::Deferred, PERSISTENCE_ID),
    ArcShared::new(SpinSyncMutex::new(Vec::new())),
    second_ready_values.clone(),
  );
  let second_system =
    TypedActorSystem::<CounterCommand>::create_from_props(&second_props, actor_system_config()).expect("second system");
  let mut second_actor = second_system.user_guardian_ref();

  assert_eq!(ask_value(&mut second_actor), 0);
  assert_eq!(*second_ready_values.lock(), vec![0]);
  terminate_system(second_system);
}

#[test]
fn domain_validation_failure_does_not_advance_persistence_sequence() {
  const PERSISTENCE_ID: &str = "typed-domain-error-counter";

  let command_log = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let ready_values = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let props =
    counter_props(counter_config(PersistenceMode::Ephemeral, PERSISTENCE_ID), command_log.clone(), ready_values);
  let system = TypedActorSystem::<CounterCommand>::create_from_props(&props, actor_system_config()).expect("system");
  let mut actor = system.user_guardian_ref();

  assert_eq!(ask_rejection(&mut actor), "domain-error");
  assert_eq!(ask_value(&mut actor), 0);
  assert_eq!(ask_sequence(&mut actor), 0);
  assert!(command_log.lock().is_empty());

  terminate_system(system);
}
