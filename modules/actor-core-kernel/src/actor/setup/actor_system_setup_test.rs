use alloc::boxed::Box;
use core::time::Duration;

use fraktor_utils_core_rs::sync::ArcShared;

use crate::{
  actor::{
    actor_ref_provider::LocalActorRefProviderInstaller,
    extension::ExtensionInstallers,
    props::MailboxConfig,
    scheduler::{SchedulerConfig, tick_driver::tests::TestTickDriver},
    setup::{ActorSystemSetup, BootstrapSetup, CircuitBreakerConfig},
  },
  dispatch::dispatcher::{
    DEFAULT_DISPATCHER_ID, DefaultDispatcherFactory, DispatcherConfig, ExecuteError, Executor, ExecutorShared,
    MessageDispatcherFactory, TrampolineState,
  },
  system::remote::RemotingConfig,
};

struct NoopExecutor;

impl Executor for NoopExecutor {
  fn execute(&mut self, _task: Box<dyn FnOnce() + Send + 'static>, _affinity_key: u64) -> Result<(), ExecuteError> {
    Ok(())
  }

  fn shutdown(&mut self) {}
}

fn dispatcher_configurator(id: &str) -> ArcShared<Box<dyn MessageDispatcherFactory>> {
  let settings = DispatcherConfig::with_defaults(id);
  let executor = ExecutorShared::new(Box::new(NoopExecutor), TrampolineState::new());
  let configurator: Box<dyn MessageDispatcherFactory> = Box::new(DefaultDispatcherFactory::new(&settings, executor));
  ArcShared::new(configurator)
}

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
fn into_actor_system_config_preserves_bootstrap_settings() {
  let remoting = RemotingConfig::default().with_canonical_host("127.0.0.1").with_canonical_port(25520);
  let start_time = Duration::from_secs(42);
  let setup = ActorSystemSetup::new(
    BootstrapSetup::default()
      .with_system_name("setup-bootstrap")
      .with_remoting_config(remoting.clone())
      .with_start_time(start_time),
  );

  let config = setup.into_actor_system_config();

  assert_eq!(config.system_name(), "setup-bootstrap");
  assert_eq!(config.remoting_config(), Some(&remoting));
  assert_eq!(config.start_time(), Some(start_time));
}

#[test]
fn into_actor_system_config_preserves_runtime_settings() {
  let default_cb = CircuitBreakerConfig::new(3, Duration::from_secs(10));
  let named_cb = CircuitBreakerConfig::new(5, Duration::from_secs(20));
  let scheduler = SchedulerConfig::default().with_runner_api_enabled(true);
  let setup = ActorSystemSetup::new(BootstrapSetup::default().with_system_name("runtime-settings"))
    .with_scheduler_config(scheduler)
    .with_tick_driver(TestTickDriver::default())
    .with_extension_installers(ExtensionInstallers::default())
    .with_actor_ref_provider_installer(LocalActorRefProviderInstaller::default())
    .with_dispatcher_factory("custom-dispatcher", dispatcher_configurator("custom-dispatcher"))
    .with_mailbox("custom-mailbox", MailboxConfig::default())
    .with_default_circuit_breaker_config(default_cb)
    .with_named_circuit_breaker_config("payments", named_cb);

  let config = setup.into_actor_system_config();

  assert!(config.scheduler_config().runner_api_enabled());
  assert!(config.has_tick_driver());
  assert!(config.extension_installers().is_some());
  assert!(config.provider_installer().is_some());
  assert!(config.dispatchers().resolve("custom-dispatcher").is_ok());
  assert!(config.mailboxes().resolve("custom-mailbox").is_ok());
  assert_eq!(config.default_circuit_breaker_config(), default_cb);
  assert_eq!(config.circuit_breaker_config("payments"), named_cb);
}

#[test]
fn with_bootstrap_setup_preserves_runtime_settings_through_into_actor_system_config() {
  let setup =
    ActorSystemSetup::new(BootstrapSetup::default().with_system_name("before").with_start_time(Duration::from_secs(1)))
      .with_scheduler_config(SchedulerConfig::default())
      .with_tick_driver(TestTickDriver::default())
      .with_extension_installers(ExtensionInstallers::default())
      .with_actor_ref_provider_installer(LocalActorRefProviderInstaller::default())
      .with_bootstrap_setup(BootstrapSetup::default().with_system_name("after"));

  let config = setup.into_actor_system_config();
  assert_eq!(config.system_name(), "after");
  assert_eq!(config.start_time(), None);
  assert!(config.has_tick_driver());
  assert!(config.extension_installers().is_some());
  assert!(config.provider_installer().is_some());
  assert!(config.dispatchers().resolve(DEFAULT_DISPATCHER_ID).is_ok());
}
