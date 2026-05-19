use fraktor_actor_adaptor_std_rs::tick_driver::TestTickDriver;
use fraktor_actor_core_kernel_rs::{
  actor::{
    Actor, ActorContext, Pid, error::ActorError, extension::ExtensionId, messaging::AnyMessageView, props::Props,
    scheduler::SchedulerConfig, setup::ActorSystemConfig,
  },
  system::ActorSystem,
};
use fraktor_utils_core_rs::sync::SharedAccess;

use crate::{
  config::PersistenceSettings,
  extension::{PersistenceExtension, PersistenceExtensionId},
  journal::{InMemoryJournal, JournalActorConfig},
  snapshot::{InMemorySnapshotStore, SnapshotActorConfig},
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
  let config = ActorSystemConfig::new(TestTickDriver::default()).with_scheduler_config(scheduler);
  let props = Props::from_fn(|| NoopActor);
  let system = ActorSystem::create_from_props(&props, config).expect("system");
  let journal = InMemoryJournal::new();
  let snapshot = InMemorySnapshotStore::new();

  let extension = PersistenceExtension::new(&system, journal, snapshot).expect("extension should build");

  assert_ne!(extension.journal_actor_ref().pid(), Pid::new(0, 0));
  assert_ne!(extension.snapshot_actor_ref().pid(), Pid::new(0, 0));
  assert_ne!(extension.journal_actor_ref().pid(), extension.snapshot_actor_ref().pid());

  let cloned = extension.clone();
  assert_eq!(cloned.journal_actor_ref().pid(), extension.journal_actor_ref().pid());
}

#[test]
fn persistence_extension_accepts_explicit_runtime_settings() {
  let scheduler = SchedulerConfig::default().with_runner_api_enabled(true);
  let config = ActorSystemConfig::new(TestTickDriver::default()).with_scheduler_config(scheduler);
  let props = Props::from_fn(|| NoopActor);
  let system = ActorSystem::create_from_props(&props, config).expect("system");
  let journal = InMemoryJournal::new();
  let snapshot = InMemorySnapshotStore::new();
  let settings = PersistenceSettings::default()
    .with_journal_actor_config(JournalActorConfig::new(2))
    .with_snapshot_actor_config(SnapshotActorConfig::new(3));

  let extension =
    PersistenceExtension::new_with_settings(&system, journal, snapshot, settings).expect("extension should build");

  assert_ne!(extension.journal_actor_ref().pid(), Pid::new(0, 0));
  assert_ne!(extension.snapshot_actor_ref().pid(), Pid::new(0, 0));
  assert_eq!(extension.settings().journal_actor_config(), JournalActorConfig::new(2));
  assert_eq!(extension.settings().snapshot_actor_config(), SnapshotActorConfig::new(3));
}

#[test]
fn persistence_extension_id_creates_shared_extension() {
  let scheduler = SchedulerConfig::default().with_runner_api_enabled(true);
  let config = ActorSystemConfig::new(TestTickDriver::default()).with_scheduler_config(scheduler);
  let props = Props::from_fn(|| NoopActor);
  let system = ActorSystem::create_from_props(&props, config).expect("system");
  let extension_id = PersistenceExtensionId::new(InMemoryJournal::new(), InMemorySnapshotStore::new());

  let extension = extension_id.create_extension(&system);

  let journal_pid = extension.with_read(|inner| inner.journal_actor_ref().pid());
  let snapshot_pid = extension.with_read(|inner| inner.snapshot_actor_ref().pid());
  assert_ne!(journal_pid, Pid::new(0, 0));
  assert_ne!(snapshot_pid, Pid::new(0, 0));
}

#[test]
#[should_panic(expected = "persistence extension bootstrap failed")]
fn persistence_extension_id_panics_when_runtime_actor_names_are_already_taken() {
  let scheduler = SchedulerConfig::default().with_runner_api_enabled(true);
  let config = ActorSystemConfig::new(TestTickDriver::default()).with_scheduler_config(scheduler);
  let props = Props::from_fn(|| NoopActor);
  let system = ActorSystem::create_from_props(&props, config).expect("system");
  let extension_id = PersistenceExtensionId::new(InMemoryJournal::new(), InMemorySnapshotStore::new());

  drop(extension_id.create_extension(&system));
  drop(extension_id.create_extension(&system));
}
