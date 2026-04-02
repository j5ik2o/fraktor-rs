use alloc::boxed::Box;

use crate::core::kernel::{
  actor::{
    actor_ref_provider::LocalActorRefProviderInstaller,
    extension::ExtensionInstallers,
    scheduler::{SchedulerConfig, tick_driver::TickDriverConfig},
    setup::{ActorSystemSetup, BootstrapSetup},
  },
  dispatch::dispatcher::{DispatcherConfig, InlineExecutor},
};

#[test]
fn actor_system_setup_composes_bootstrap_and_runtime_settings() {
  let dispatcher = DispatcherConfig::from_executor(Box::new(InlineExecutor::new()));
  let tick_driver =
    TickDriverConfig::manual(crate::core::kernel::actor::scheduler::tick_driver::ManualTestDriver::new());
  let setup = ActorSystemSetup::new(BootstrapSetup::default().with_system_name("setup-system"))
    .with_scheduler_config(SchedulerConfig::default())
    .with_tick_driver(tick_driver)
    .with_extension_installers(ExtensionInstallers::default())
    .with_actor_ref_provider_installer(LocalActorRefProviderInstaller::default())
    .with_default_dispatcher(dispatcher);

  let config = setup.as_actor_system_config();
  assert_eq!(config.system_name(), "setup-system");
  assert!(config.tick_driver_config().is_some());
  assert!(config.extension_installers().is_some());
  assert!(config.provider_installer().is_some());
  assert!(config.default_dispatcher_config().is_some());
}
