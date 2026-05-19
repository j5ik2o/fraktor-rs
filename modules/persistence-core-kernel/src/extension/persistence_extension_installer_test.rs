use fraktor_actor_adaptor_std_rs::tick_driver::TestTickDriver;
use fraktor_actor_core_kernel_rs::{
  actor::{
    Actor, ActorContext, error::ActorError, extension::ExtensionInstallers, messaging::AnyMessageView, props::Props,
    scheduler::SchedulerConfig, setup::ActorSystemConfig,
  },
  system::ActorSystem,
};

use crate::{
  config::PersistenceSettings,
  extension::{PersistenceExtensionInstaller, PersistenceExtensionShared},
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
fn installer_registers_persistence_extension() {
  let journal = InMemoryJournal::new();
  let snapshot_store = InMemorySnapshotStore::new();
  let installer = PersistenceExtensionInstaller::new(journal, snapshot_store);
  let installers = ExtensionInstallers::default().with_extension_installer(installer);
  let scheduler = SchedulerConfig::default().with_runner_api_enabled(true);
  let config = ActorSystemConfig::new(TestTickDriver::default())
    .with_scheduler_config(scheduler)
    .with_extension_installers(installers);
  let props = Props::from_fn(|| NoopActor);
  let system = ActorSystem::create_from_props(&props, config).expect("system");

  let extension = system.extended().extension_by_type::<PersistenceExtensionShared>();

  assert!(extension.is_some());
}

#[test]
fn installer_registers_extension_with_explicit_settings() {
  let journal = InMemoryJournal::new();
  let snapshot_store = InMemorySnapshotStore::new();
  let settings = PersistenceSettings::default()
    .with_journal_actor_config(JournalActorConfig::new(2))
    .with_snapshot_actor_config(SnapshotActorConfig::new(3));
  let installer = PersistenceExtensionInstaller::new_with_settings(journal, snapshot_store, settings);
  let installers = ExtensionInstallers::default().with_extension_installer(installer);
  let scheduler = SchedulerConfig::default().with_runner_api_enabled(true);
  let config = ActorSystemConfig::new(TestTickDriver::default())
    .with_scheduler_config(scheduler)
    .with_extension_installers(installers);
  let props = Props::from_fn(|| NoopActor);
  let system = ActorSystem::create_from_props(&props, config).expect("system");

  let extension = system.extended().extension_by_type::<PersistenceExtensionShared>();

  assert!(extension.is_some());
}
