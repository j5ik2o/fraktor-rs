use fraktor_actor_adaptor_std_rs::tick_driver::TestTickDriver;
use fraktor_actor_core_kernel_rs::{
  actor::{
    Actor, ActorContext, error::ActorError, extension::ExtensionInstallers, messaging::AnyMessageView, props::Props,
    scheduler::SchedulerConfig, setup::ActorSystemConfig,
  },
  system::ActorSystem,
};

use crate::{
  in_memory_journal::InMemoryJournal, in_memory_snapshot_store::InMemorySnapshotStore,
  persistence_extension_installer::PersistenceExtensionInstaller,
  persistence_extension_shared::PersistenceExtensionShared,
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
