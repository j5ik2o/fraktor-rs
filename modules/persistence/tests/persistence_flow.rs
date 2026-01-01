//! Persistent actor flow integration tests.

extern crate alloc;

use alloc::{string::String, vec::Vec};
use core::{
  future::Future,
  task::{Context, Poll, RawWaker, RawWakerVTable, Waker},
};

use fraktor_actor_rs::core::{
  actor::{Actor, ActorContextGeneric, actor_ref::ActorRefGeneric},
  error::ActorError,
  messaging::{AnyMessageGeneric, AnyMessageViewGeneric},
  props::PropsGeneric,
  scheduler::{ManualTestDriver, SchedulerConfig, TickDriverConfig},
  system::{ActorSystemConfigGeneric, ActorSystemGeneric},
};
use fraktor_persistence_rs::core::{
  Eventsourced, InMemoryJournal, InMemorySnapshotStore, Journal, PersistenceContext, PersistenceExtensionInstaller,
  PersistentActor, PersistentRepr, Snapshot, SnapshotMetadata, SnapshotStore, persistent_props, spawn_persistent,
};
use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdToolbox, RuntimeToolbox, SyncMutexFamily, ToolboxMutex},
  sync::ArcShared,
};

type TB = NoStdToolbox;
type SharedValue = ArcShared<ToolboxMutex<i32, TB>>;
type SharedFlag = ArcShared<ToolboxMutex<bool, TB>>;
type SharedRefs = ArcShared<ToolboxMutex<Vec<ActorRefGeneric<TB>>, TB>>;

#[derive(Clone)]
enum Command {
  Add(i32),
}

#[derive(Clone)]
enum Event {
  Incremented(i32),
}

struct CounterActor {
  context:           PersistenceContext<CounterActor, TB>,
  value:             SharedValue,
  recovery_complete: SharedFlag,
}

impl CounterActor {
  fn new(persistence_id: &str, value: SharedValue, recovery_complete: SharedFlag) -> Self {
    Self { context: PersistenceContext::new(persistence_id.to_string()), value, recovery_complete }
  }

  fn apply_event(&mut self, event: &Event) {
    let Event::Incremented(delta) = event;
    let mut guard = self.value.lock();
    *guard += delta;
  }
}

impl Eventsourced<TB> for CounterActor {
  fn persistence_id(&self) -> &str {
    self.context.persistence_id()
  }

  fn receive_recover(&mut self, repr: &PersistentRepr) {
    if let Some(event) = repr.downcast_ref::<Event>() {
      self.apply_event(event);
    }
  }

  fn receive_snapshot(&mut self, snapshot: &Snapshot) {
    if let Some(value) = snapshot.data().downcast_ref::<i32>() {
      *self.value.lock() = *value;
    }
  }

  fn receive_command(
    &mut self,
    ctx: &mut ActorContextGeneric<'_, TB>,
    message: AnyMessageViewGeneric<'_, TB>,
  ) -> Result<(), ActorError> {
    if let Some(Command::Add(delta)) = message.downcast_ref::<Command>() {
      self.persist(ctx, Event::Incremented(*delta), |actor, event| actor.apply_event(event));
      self.flush_batch(ctx);
    }
    Ok(())
  }

  fn on_recovery_completed(&mut self) {
    *self.recovery_complete.lock() = true;
  }

  fn last_sequence_nr(&self) -> u64 {
    self.context.last_sequence_nr()
  }
}

impl PersistentActor<TB> for CounterActor {
  fn persistence_context(&mut self) -> &mut PersistenceContext<Self, TB> {
    &mut self.context
  }
}

#[derive(Clone)]
struct ActorSetup {
  persistence_id:    String,
  value:             SharedValue,
  recovery_complete: SharedFlag,
}

struct Guardian {
  setups:     Vec<ActorSetup>,
  child_refs: SharedRefs,
}

impl Guardian {
  fn new(setups: Vec<ActorSetup>, child_refs: SharedRefs) -> Self {
    Self { setups, child_refs }
  }
}

impl Actor<TB> for Guardian {
  fn receive(
    &mut self,
    ctx: &mut ActorContextGeneric<'_, TB>,
    message: AnyMessageViewGeneric<'_, TB>,
  ) -> Result<(), ActorError> {
    if message.downcast_ref::<Start>().is_none() {
      return Ok(());
    }

    let mut refs = self.child_refs.lock();
    for setup in self.setups.iter() {
      let value = setup.value.clone();
      let recovery_complete = setup.recovery_complete.clone();
      let persistence_id = setup.persistence_id.clone();
      let props =
        persistent_props(move || CounterActor::new(&persistence_id, value.clone(), recovery_complete.clone()));
      let child = spawn_persistent(ctx, &props)
        .map_err(|error| ActorError::recoverable(format!("spawn persistent actor failed: {error:?}")))?;
      refs.push(child);
    }
    Ok(())
  }
}

struct Start;

fn shared_mutex<T: Send + 'static>(value: T) -> ArcShared<ToolboxMutex<T, TB>> {
  ArcShared::new(<<NoStdToolbox as RuntimeToolbox>::MutexFamily as SyncMutexFamily>::create(value))
}

fn drive_ready<F: Future>(future: F) -> F::Output {
  let waker = unsafe { Waker::from_raw(raw_waker()) };
  let mut context = Context::from_waker(&waker);
  let mut future = core::pin::pin!(future);
  match Future::poll(future.as_mut(), &mut context) {
    | Poll::Ready(output) => output,
    | Poll::Pending => panic!("future was not ready"),
  }
}

fn raw_waker() -> RawWaker {
  RawWaker::new(core::ptr::null(), &RAW_WAKER_VTABLE)
}

static RAW_WAKER_VTABLE: RawWakerVTable = RawWakerVTable::new(clone_raw, wake_raw, wake_raw, drop_raw);

unsafe fn clone_raw(_: *const ()) -> RawWaker {
  raw_waker()
}

const unsafe fn wake_raw(_: *const ()) {}

const unsafe fn drop_raw(_: *const ()) {}

#[test]
fn recovery_flow_snapshot_then_replay() {
  let value = shared_mutex(0);
  let recovery_complete = shared_mutex(false);
  let refs = shared_mutex(Vec::new());
  let setups = vec![ActorSetup {
    persistence_id:    "pid-1".to_string(),
    value:             value.clone(),
    recovery_complete: recovery_complete.clone(),
  }];

  let mut journal = InMemoryJournal::new();
  let repr0 = PersistentRepr::new("pid-1", 1, ArcShared::new(Event::Incremented(4)));
  let repr1 = PersistentRepr::new("pid-1", 2, ArcShared::new(Event::Incremented(6)));
  let repr2 = PersistentRepr::new("pid-1", 3, ArcShared::new(Event::Incremented(2)));
  let repr3 = PersistentRepr::new("pid-1", 4, ArcShared::new(Event::Incremented(3)));
  let _ = drive_ready(journal.write_messages(&[repr0, repr1, repr2, repr3]));

  let mut snapshot_store = InMemorySnapshotStore::new();
  let snapshot_metadata = SnapshotMetadata::new("pid-1", 2, 0);
  let snapshot_payload: ArcShared<dyn core::any::Any + Send + Sync> = ArcShared::new(10_i32);
  let _ = drive_ready(snapshot_store.save_snapshot(snapshot_metadata, snapshot_payload));

  let installer = PersistenceExtensionInstaller::new(journal, snapshot_store);
  let installers =
    fraktor_actor_rs::core::extension::ExtensionInstallers::default().with_extension_installer(installer);
  let scheduler = SchedulerConfig::default().with_runner_api_enabled(true);
  let tick_driver = TickDriverConfig::manual(ManualTestDriver::new());
  let config = ActorSystemConfigGeneric::default()
    .with_scheduler_config(scheduler)
    .with_tick_driver(tick_driver)
    .with_extension_installers(installers);
  let props = PropsGeneric::from_fn({
    let setups = setups.clone();
    let refs = refs.clone();
    move || Guardian::new(setups.clone(), refs.clone())
  });
  let system = ActorSystemGeneric::<TB>::new_with_config(&props, &config).expect("system");
  let controller = system.tick_driver_bundle().manual_controller().expect("manual controller").clone();

  system.user_guardian_ref().tell(AnyMessageGeneric::new(Start)).expect("start");

  for _ in 0..50 {
    controller.inject_and_drive(1);
    if *recovery_complete.lock() {
      break;
    }
  }

  assert!(*recovery_complete.lock());
  assert_eq!(*value.lock(), 15);
  assert_eq!(refs.lock().len(), 1);
}

#[test]
fn persist_flow_keeps_values_independent() {
  let value_a = shared_mutex(0);
  let value_b = shared_mutex(0);
  let recovery_a = shared_mutex(false);
  let recovery_b = shared_mutex(false);
  let refs = shared_mutex(Vec::new());
  let setups = vec![
    ActorSetup {
      persistence_id:    "pid-a".to_string(),
      value:             value_a.clone(),
      recovery_complete: recovery_a,
    },
    ActorSetup {
      persistence_id:    "pid-b".to_string(),
      value:             value_b.clone(),
      recovery_complete: recovery_b,
    },
  ];

  let installer = PersistenceExtensionInstaller::new(InMemoryJournal::new(), InMemorySnapshotStore::new());
  let installers =
    fraktor_actor_rs::core::extension::ExtensionInstallers::default().with_extension_installer(installer);
  let scheduler = SchedulerConfig::default().with_runner_api_enabled(true);
  let tick_driver = TickDriverConfig::manual(ManualTestDriver::new());
  let config = ActorSystemConfigGeneric::default()
    .with_scheduler_config(scheduler)
    .with_tick_driver(tick_driver)
    .with_extension_installers(installers);
  let props = PropsGeneric::from_fn({
    let setups = setups.clone();
    let refs = refs.clone();
    move || Guardian::new(setups.clone(), refs.clone())
  });
  let system = ActorSystemGeneric::<TB>::new_with_config(&props, &config).expect("system");
  let controller = system.tick_driver_bundle().manual_controller().expect("manual controller").clone();

  system.user_guardian_ref().tell(AnyMessageGeneric::new(Start)).expect("start");

  for _ in 0..5 {
    controller.inject_and_drive(1);
  }

  let refs_guard = refs.lock();
  assert_eq!(refs_guard.len(), 2);
  refs_guard[0].tell(AnyMessageGeneric::new(Command::Add(2))).expect("send add");
  refs_guard[1].tell(AnyMessageGeneric::new(Command::Add(5))).expect("send add");
  drop(refs_guard);

  for _ in 0..10 {
    controller.inject_and_drive(1);
  }

  assert_eq!(*value_a.lock(), 2);
  assert_eq!(*value_b.lock(), 5);
}
