use core::time::Duration;

use crate::core::kernel::{
  actor::{
    actor_ref_provider::LocalActorRefProviderInstaller,
    extension::ExtensionInstallers,
    scheduler::{SchedulerConfig, tick_driver::TestTickDriver},
    setup::{ActorSystemSetup, BootstrapSetup},
  },
  dispatch::dispatcher::DEFAULT_DISPATCHER_ID,
};

#[test]
fn actor_system_setup_composes_bootstrap_and_runtime_settings() {
  let setup = ActorSystemSetup::new(BootstrapSetup::default().with_system_name("setup-system"))
    .with_scheduler_config(SchedulerConfig::default())
    .with_tick_driver(TestTickDriver::default())
    .with_extension_installers(ExtensionInstallers::default())
    .with_actor_ref_provider_installer(LocalActorRefProviderInstaller::default());

  let config = setup.as_actor_system_config();
  assert_eq!(config.system_name(), "setup-system");
  assert!(config.has_tick_driver());
  assert!(config.extension_installers().is_some());
  assert!(config.provider_installer().is_some());
  assert!(config.dispatchers().resolve(DEFAULT_DISPATCHER_ID).is_ok());
}

#[test]
fn with_bootstrap_setup_preserves_runtime_settings() {
  let setup =
    ActorSystemSetup::new(BootstrapSetup::default().with_system_name("before").with_start_time(Duration::from_secs(1)))
      .with_scheduler_config(SchedulerConfig::default())
      .with_tick_driver(TestTickDriver::default())
      .with_extension_installers(ExtensionInstallers::default())
      .with_actor_ref_provider_installer(LocalActorRefProviderInstaller::default())
      .with_bootstrap_setup(BootstrapSetup::default().with_system_name("after"));

  let config = setup.as_actor_system_config();
  assert_eq!(config.system_name(), "after");
  assert_eq!(config.start_time(), None);
  assert!(config.has_tick_driver());
  assert!(config.extension_installers().is_some());
  assert!(config.provider_installer().is_some());
  assert!(config.dispatchers().resolve(DEFAULT_DISPATCHER_ID).is_ok());
}
