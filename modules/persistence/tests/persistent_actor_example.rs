//! Persistent actor example integration test.

extern crate alloc;

use alloc::vec::Vec;

use fraktor_actor_rs::core::{
  actor::{Actor, ActorContextGeneric, actor_ref::ActorRefGeneric},
  error::ActorError,
  messaging::{AnyMessageGeneric, AnyMessageViewGeneric},
  props::PropsGeneric,
  scheduler::{
    SchedulerConfig,
    tick_driver::{ManualTestDriver, TickDriverConfig},
  },
  system::{ActorSystemConfigGeneric, ActorSystemGeneric},
};
use fraktor_persistence_rs::core::{
  Eventsourced, InMemoryJournal, InMemorySnapshotStore, PersistenceContext, PersistenceExtensionInstaller,
  PersistentActor, PersistentRepr, Snapshot, persistent_props, spawn_persistent,
};
use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdToolbox, RuntimeToolbox, ToolboxMutex, sync_mutex_family::SyncMutexFamily},
  sync::ArcShared,
};

type TB = NoStdToolbox;
type SharedValue = ArcShared<ToolboxMutex<i32, TB>>;
type SharedRefs = ArcShared<ToolboxMutex<Vec<ActorRefGeneric<TB>>, TB>>;

#[derive(Clone)]
enum Command {
  AddAll(Vec<i32>),
}

#[derive(Clone)]
enum Event {
  Added(i32),
}

struct BatchActor {
  context: PersistenceContext<BatchActor, TB>,
  value:   SharedValue,
}

impl BatchActor {
  fn new(persistence_id: &str, value: SharedValue) -> Self {
    Self { context: PersistenceContext::new(persistence_id.to_string()), value }
  }

  fn apply_event(&mut self, event: &Event) {
    let Event::Added(delta) = event;
    let mut guard = self.value.lock();
    *guard += delta;
  }
}

impl Eventsourced<TB> for BatchActor {
  fn persistence_id(&self) -> &str {
    self.context.persistence_id()
  }

  fn receive_recover(&mut self, repr: &PersistentRepr) {
    if let Some(event) = repr.downcast_ref::<Event>() {
      self.apply_event(event);
    }
  }

  fn receive_snapshot(&mut self, _snapshot: &Snapshot) {}

  fn receive_command(
    &mut self,
    ctx: &mut ActorContextGeneric<'_, TB>,
    message: AnyMessageViewGeneric<'_, TB>,
  ) -> Result<(), ActorError> {
    if let Some(Command::AddAll(events)) = message.downcast_ref::<Command>() {
      let mapped: Vec<Event> = events.iter().map(|value| Event::Added(*value)).collect();
      self.persist_all(ctx, mapped, |actor, event| actor.apply_event(event));
      self.flush_batch(ctx)?;
    }
    Ok(())
  }

  fn last_sequence_nr(&self) -> u64 {
    self.context.last_sequence_nr()
  }
}

impl PersistentActor<TB> for BatchActor {
  fn persistence_context(&mut self) -> &mut PersistenceContext<Self, TB> {
    &mut self.context
  }
}

struct Guardian {
  value:      SharedValue,
  child_refs: SharedRefs,
}

impl Guardian {
  fn new(value: SharedValue, child_refs: SharedRefs) -> Self {
    Self { value, child_refs }
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
    let value = self.value.clone();
    let child_refs = self.child_refs.clone();
    let props = persistent_props(move || BatchActor::new("batch-1", value.clone()));
    let child =
      spawn_persistent(ctx, &props).map_err(|error| ActorError::recoverable(format!("spawn failed: {error:?}")))?;
    child_refs.lock().push(child);
    Ok(())
  }
}

struct Start;

fn shared_mutex<T: Send + 'static>(value: T) -> ArcShared<ToolboxMutex<T, TB>> {
  ArcShared::new(<<NoStdToolbox as RuntimeToolbox>::MutexFamily as SyncMutexFamily>::create(value))
}

#[test]
fn batch_flow_applies_all_events() {
  let value = shared_mutex(0);
  let child_refs = shared_mutex(Vec::new());
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
    let value = value.clone();
    let child_refs = child_refs.clone();
    move || Guardian::new(value.clone(), child_refs.clone())
  });
  let system = ActorSystemGeneric::<TB>::new_with_config(&props, &config).expect("system");
  let controller = system.tick_driver_bundle().manual_controller().expect("manual controller").clone();

  system.user_guardian_ref().tell(AnyMessageGeneric::new(Start)).expect("start");

  for _ in 0..20 {
    controller.inject_and_drive(1);
    if !child_refs.lock().is_empty() {
      break;
    }
  }

  if let Some(child) = child_refs.lock().first().cloned() {
    let _ = child.tell(AnyMessageGeneric::new(Command::AddAll(vec![1, 2, 3])));
  }

  for _ in 0..10 {
    controller.inject_and_drive(1);
  }

  assert_eq!(*value.lock(), 6);
}
