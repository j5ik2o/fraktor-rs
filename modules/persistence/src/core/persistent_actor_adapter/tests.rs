use alloc::string::ToString;

use fraktor_actor_rs::core::{
  actor::{Actor, ActorCellGeneric, ActorContextGeneric, actor_ref::ActorRefGeneric},
  error::ActorError,
  messaging::AnyMessageViewGeneric,
  props::PropsGeneric,
  scheduler::{
    SchedulerConfig,
    tick_driver::{ManualTestDriver, TickDriverConfig},
  },
  system::{
    ActorSystemConfigGeneric, ActorSystemGeneric,
    state::{SystemStateSharedGeneric, system_state::SystemStateGeneric},
  },
};
use fraktor_utils_rs::core::runtime_toolbox::NoStdToolbox;

use crate::core::{
  eventsourced::Eventsourced, in_memory_journal::InMemoryJournal, in_memory_snapshot_store::InMemorySnapshotStore,
  persistence_context::PersistenceContext, persistence_extension_installer::PersistenceExtensionInstaller,
  persistent_actor::PersistentActor, persistent_actor_adapter::PersistentActorAdapter, persistent_repr::PersistentRepr,
  snapshot::Snapshot,
};

type TB = NoStdToolbox;

struct NoopActor;

impl Actor<TB> for NoopActor {
  fn receive(
    &mut self,
    _ctx: &mut ActorContextGeneric<'_, TB>,
    _message: AnyMessageViewGeneric<'_, TB>,
  ) -> Result<(), ActorError> {
    Ok(())
  }
}

struct DummyPersistentActor {
  context: PersistenceContext<DummyPersistentActor, TB>,
}

impl DummyPersistentActor {
  fn new() -> Self {
    Self { context: PersistenceContext::new("pid-1".to_string()) }
  }
}

impl Eventsourced<TB> for DummyPersistentActor {
  fn persistence_id(&self) -> &str {
    self.context.persistence_id()
  }

  fn receive_recover(&mut self, _event: &PersistentRepr) {}

  fn receive_snapshot(&mut self, _snapshot: &Snapshot) {}

  fn receive_command(
    &mut self,
    _ctx: &mut ActorContextGeneric<'_, TB>,
    _message: AnyMessageViewGeneric<'_, TB>,
  ) -> Result<(), ActorError> {
    Ok(())
  }

  fn last_sequence_nr(&self) -> u64 {
    self.context.last_sequence_nr()
  }
}

impl PersistentActor<TB> for DummyPersistentActor {
  fn persistence_context(&mut self) -> &mut PersistenceContext<Self, TB> {
    &mut self.context
  }
}

struct MismatchPersistentActor {
  context: PersistenceContext<MismatchPersistentActor, TB>,
}

impl MismatchPersistentActor {
  fn new() -> Self {
    Self { context: PersistenceContext::new("pid-other".to_string()) }
  }
}

impl Eventsourced<TB> for MismatchPersistentActor {
  fn persistence_id(&self) -> &str {
    "pid-1"
  }

  fn receive_recover(&mut self, _event: &PersistentRepr) {}

  fn receive_snapshot(&mut self, _snapshot: &Snapshot) {}

  fn receive_command(
    &mut self,
    _ctx: &mut ActorContextGeneric<'_, TB>,
    _message: AnyMessageViewGeneric<'_, TB>,
  ) -> Result<(), ActorError> {
    Ok(())
  }

  fn last_sequence_nr(&self) -> u64 {
    self.context.last_sequence_nr()
  }
}

impl PersistentActor<TB> for MismatchPersistentActor {
  fn persistence_context(&mut self) -> &mut PersistenceContext<Self, TB> {
    &mut self.context
  }
}

fn build_context(system: &ActorSystemGeneric<TB>) -> ActorContextGeneric<'static, TB> {
  let pid = system.allocate_pid();
  let props = PropsGeneric::from_fn(|| NoopActor);
  let cell =
    ActorCellGeneric::create(system.state(), pid, None, "test".into(), &props).expect("actor cell should be created");
  system.state().register_cell(cell);
  ActorContextGeneric::new(system, pid)
}

#[test]
fn adapter_pre_start_fails_without_extension() {
  let system = ActorSystemGeneric::<TB>::from_state(SystemStateSharedGeneric::new(SystemStateGeneric::new()));
  let mut ctx = build_context(&system);
  let actor = DummyPersistentActor::new();
  let mut adapter = PersistentActorAdapter::<_, TB>::new(actor);

  let result = adapter.pre_start(&mut ctx);
  assert!(matches!(result, Err(ActorError::Fatal(_))));
}

#[test]
fn adapter_pre_start_binds_context() {
  let journal = InMemoryJournal::new();
  let snapshot_store = InMemorySnapshotStore::new();
  let installer = PersistenceExtensionInstaller::new(journal, snapshot_store);
  let installers =
    fraktor_actor_rs::core::extension::ExtensionInstallers::default().with_extension_installer(installer);
  let scheduler = SchedulerConfig::default().with_runner_api_enabled(true);
  let tick_driver = TickDriverConfig::manual(ManualTestDriver::new());
  let config = ActorSystemConfigGeneric::default()
    .with_scheduler_config(scheduler)
    .with_tick_driver(tick_driver)
    .with_extension_installers(installers);
  let props = PropsGeneric::from_fn(|| NoopActor);
  let system = ActorSystemGeneric::<TB>::new_with_config(&props, &config).expect("system");

  let mut ctx = build_context(&system);
  let actor = DummyPersistentActor::new();
  let mut adapter = PersistentActorAdapter::<_, TB>::new(actor);

  adapter.pre_start(&mut ctx).expect("pre_start");

  let result = adapter.actor.persistence_context().bind_actor_refs(ActorRefGeneric::null(), ActorRefGeneric::null());
  assert!(result.is_err());
}

#[test]
fn adapter_pre_start_rejects_persistence_id_mismatch() {
  let journal = InMemoryJournal::new();
  let snapshot_store = InMemorySnapshotStore::new();
  let installer = PersistenceExtensionInstaller::new(journal, snapshot_store);
  let installers =
    fraktor_actor_rs::core::extension::ExtensionInstallers::default().with_extension_installer(installer);
  let scheduler = SchedulerConfig::default().with_runner_api_enabled(true);
  let tick_driver = TickDriverConfig::manual(ManualTestDriver::new());
  let config = ActorSystemConfigGeneric::default()
    .with_scheduler_config(scheduler)
    .with_tick_driver(tick_driver)
    .with_extension_installers(installers);
  let props = PropsGeneric::from_fn(|| NoopActor);
  let system = ActorSystemGeneric::<TB>::new_with_config(&props, &config).expect("system");

  let mut ctx = build_context(&system);
  let actor = MismatchPersistentActor::new();
  let mut adapter = PersistentActorAdapter::<_, TB>::new(actor);

  let result = adapter.pre_start(&mut ctx);
  assert!(matches!(result, Err(ActorError::Fatal(_))));
}
