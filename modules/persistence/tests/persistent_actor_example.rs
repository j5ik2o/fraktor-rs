//! Persistent actor example integration test.

mod test_utils;

extern crate alloc;

use alloc::vec::Vec;

use fraktor_actor_core_rs::core::kernel::{
  actor::{
    Actor, ActorContext,
    actor_ref::ActorRef,
    error::ActorError,
    messaging::{AnyMessage, AnyMessageView},
    props::Props,
    scheduler::{
      SchedulerConfig,
      tick_driver::{ManualTestDriver, TickDriverConfig},
    },
    setup::ActorSystemConfig,
  },
  system::ActorSystem,
};
use fraktor_persistence_rs::core::{
  Eventsourced, InMemoryJournal, InMemorySnapshotStore, PersistenceContext, PersistenceExtensionInstaller,
  PersistentActor, PersistentRepr, Snapshot, persistent_props, spawn_persistent,
};
use fraktor_utils_rs::core::sync::{ArcShared, RuntimeMutex};
use test_utils::shared_mutex;
type SharedValue = ArcShared<RuntimeMutex<i32>>;
type SharedRefs = ArcShared<RuntimeMutex<Vec<ActorRef>>>;

#[derive(Clone)]
enum Command {
  AddAll(Vec<i32>),
}

#[derive(Clone)]
enum Event {
  Added(i32),
}

struct BatchActor {
  context: PersistenceContext<BatchActor>,
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

impl Eventsourced for BatchActor {
  fn persistence_id(&self) -> &str {
    self.context.persistence_id()
  }

  fn receive_recover(&mut self, repr: &PersistentRepr) {
    if let Some(event) = repr.downcast_ref::<Event>() {
      self.apply_event(event);
    }
  }

  fn receive_snapshot(&mut self, _snapshot: &Snapshot) {}

  fn receive_command(&mut self, ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
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

impl PersistentActor for BatchActor {
  fn persistence_context(&mut self) -> &mut PersistenceContext<Self> {
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

impl Actor for Guardian {
  fn receive(&mut self, ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
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

#[test]
fn batch_flow_applies_all_events() {
  let value = shared_mutex(0);
  let child_refs = shared_mutex(Vec::new());
  let installer = PersistenceExtensionInstaller::new(InMemoryJournal::new(), InMemorySnapshotStore::new());
  let installers = fraktor_actor_core_rs::core::kernel::actor::extension::ExtensionInstallers::default()
    .with_extension_installer(installer);
  let scheduler = SchedulerConfig::default().with_runner_api_enabled(true);
  let tick_driver = TickDriverConfig::manual(ManualTestDriver::new());
  let config = ActorSystemConfig::default()
    .with_scheduler_config(scheduler)
    .with_tick_driver(tick_driver)
    .with_extension_installers(installers);
  let props = Props::from_fn({
    let value = value.clone();
    let child_refs = child_refs.clone();
    move || Guardian::new(value.clone(), child_refs.clone())
  });
  let system = ActorSystem::new_with_config(&props, &config).expect("system");
  let controller = system.tick_driver_bundle().manual_controller().expect("manual controller").clone();

  system.user_guardian_ref().tell(AnyMessage::new(Start));

  for _ in 0..20 {
    controller.inject_and_drive(1);
    if !child_refs.lock().is_empty() {
      break;
    }
  }

  if let Some(mut child) = child_refs.lock().first().cloned() {
    child.tell(AnyMessage::new(Command::AddAll(vec![1, 2, 3])));
  }

  for _ in 0..10 {
    controller.inject_and_drive(1);
  }

  assert_eq!(*value.lock(), 6);
}
