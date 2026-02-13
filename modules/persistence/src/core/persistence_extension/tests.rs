use fraktor_actor_rs::core::{
  actor::{Actor, ActorContextGeneric, Pid},
  error::ActorError,
  messaging::AnyMessageViewGeneric,
  props::PropsGeneric,
  scheduler::{
    SchedulerConfig,
    tick_driver::{ManualTestDriver, TickDriverConfig},
  },
  system::{ActorSystemConfigGeneric, ActorSystemGeneric},
};
use fraktor_utils_rs::core::runtime_toolbox::NoStdToolbox;

use crate::core::{
  in_memory_journal::InMemoryJournal, in_memory_snapshot_store::InMemorySnapshotStore,
  persistence_extension::PersistenceExtensionGeneric,
};

struct NoopActor;

impl Actor<NoStdToolbox> for NoopActor {
  fn receive(
    &mut self,
    _ctx: &mut ActorContextGeneric<'_, NoStdToolbox>,
    _message: AnyMessageViewGeneric<'_, NoStdToolbox>,
  ) -> Result<(), ActorError> {
    Ok(())
  }
}

#[test]
fn persistence_extension_creates_actor_refs() {
  let scheduler = SchedulerConfig::default().with_runner_api_enabled(true);
  let tick_driver = TickDriverConfig::manual(ManualTestDriver::new());
  let config = ActorSystemConfigGeneric::default().with_scheduler_config(scheduler).with_tick_driver(tick_driver);
  let props = PropsGeneric::from_fn(|| NoopActor);
  let system = ActorSystemGeneric::<NoStdToolbox>::new_with_config(&props, &config).expect("system");
  let journal = InMemoryJournal::new();
  let snapshot = InMemorySnapshotStore::new();

  let extension = PersistenceExtensionGeneric::new(&system, journal, snapshot).expect("extension should build");

  assert_ne!(extension.journal_actor_ref().pid(), Pid::new(0, 0));
  assert_ne!(extension.snapshot_actor_ref().pid(), Pid::new(0, 0));
  assert_ne!(extension.journal_actor_ref().pid(), extension.snapshot_actor_ref().pid());

  let cloned = extension.clone();
  assert_eq!(cloned.journal_actor_ref().pid(), extension.journal_actor_ref().pid());
}
