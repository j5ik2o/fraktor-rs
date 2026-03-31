use fraktor_actor_rs::core::kernel::{
  actor::{
    Actor, ActorContext, Pid,
    error::ActorError,
    messaging::AnyMessageView,
    props::Props,
    scheduler::{
      SchedulerConfig,
      tick_driver::{ManualTestDriver, TickDriverConfig},
    },
    setup::ActorSystemConfig,
  },
  system::ActorSystem,
};

use crate::core::{
  in_memory_journal::InMemoryJournal, in_memory_snapshot_store::InMemorySnapshotStore,
  persistence_extension::PersistenceExtension,
};

struct NoopActor;

impl Actor for NoopActor {
  fn receive(&mut self, _ctx: &mut ActorContext<'_>, _message: AnyMessageView<'_>) -> Result<(), ActorError> {
    Ok(())
  }
}

#[test]
fn persistence_extension_creates_actor_refs() {
  let scheduler = SchedulerConfig::default().with_runner_api_enabled(true);
  let tick_driver = TickDriverConfig::manual(ManualTestDriver::new());
  let config = ActorSystemConfig::default().with_scheduler_config(scheduler).with_tick_driver(tick_driver);
  let props = Props::from_fn(|| NoopActor);
  let system = ActorSystem::new_with_config(&props, &config).expect("system");
  let journal = InMemoryJournal::new();
  let snapshot = InMemorySnapshotStore::new();

  let extension = PersistenceExtension::new(&system, journal, snapshot).expect("extension should build");

  assert_ne!(extension.journal_actor_ref().pid(), Pid::new(0, 0));
  assert_ne!(extension.snapshot_actor_ref().pid(), Pid::new(0, 0));
  assert_ne!(extension.journal_actor_ref().pid(), extension.snapshot_actor_ref().pid());

  let cloned = extension.clone();
  assert_eq!(cloned.journal_actor_ref().pid(), extension.journal_actor_ref().pid());
}
