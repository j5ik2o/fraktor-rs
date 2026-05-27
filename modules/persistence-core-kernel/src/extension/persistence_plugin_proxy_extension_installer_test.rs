use fraktor_actor_adaptor_std_rs::tick_driver::TestTickDriver;
use fraktor_actor_core_kernel_rs::{
  actor::{extension::ExtensionInstallers, scheduler::SchedulerConfig, setup::ActorSystemConfig},
  system::ActorSystem,
};
use fraktor_utils_core_rs::sync::SharedAccess;

use crate::{
  config::PersistenceSettings,
  extension::{PersistenceExtensionShared, PersistencePluginProxyExtensionInstaller},
  journal::JournalActorConfig,
  snapshot::SnapshotActorConfig,
};

fn build_system(installers: ExtensionInstallers) -> ActorSystem {
  let scheduler = SchedulerConfig::default().with_runner_api_enabled(true);
  let config = ActorSystemConfig::new(TestTickDriver::default())
    .with_scheduler_config(scheduler)
    .with_extension_installers(installers);
  ActorSystem::create_with_noop_guardian(config).expect("system")
}

#[test]
fn proxy_installer_registers_persistence_extension() {
  let installers =
    ExtensionInstallers::default().with_extension_installer(PersistencePluginProxyExtensionInstaller::new());
  let system = build_system(installers);

  let extension = system.extended().extension_by_type::<PersistenceExtensionShared>();

  assert!(extension.is_some());
}

#[test]
fn proxy_installer_preserves_explicit_settings() {
  let settings = PersistenceSettings::default()
    .with_journal_actor_config(JournalActorConfig::new(2))
    .with_snapshot_actor_config(SnapshotActorConfig::new(3));
  let installers = ExtensionInstallers::default()
    .with_extension_installer(PersistencePluginProxyExtensionInstaller::new_with_settings(settings));
  let system = build_system(installers);
  let extension = system.extended().extension_by_type::<PersistenceExtensionShared>().expect("extension");

  extension.with_read(|inner| {
    assert_eq!(inner.settings().journal_actor_config(), JournalActorConfig::new(2));
    assert_eq!(inner.settings().snapshot_actor_config(), SnapshotActorConfig::new(3));
  });
}
