//! Actor-system helpers for Embassy environments.

use alloc::boxed::Box;

use fraktor_actor_core_kernel_rs::{
  actor::setup::ActorSystemConfig,
  dispatch::dispatcher::{
    DEFAULT_BLOCKING_DISPATCHER_ID, DEFAULT_DISPATCHER_ID, DefaultDispatcherFactory, DispatcherConfig, ExecutorFactory,
    MessageDispatcherFactory,
  },
};
use fraktor_utils_core_rs::sync::ArcShared;

use crate::{dispatch::EmbassyExecutorFactory, tick_driver::EmbassyTickDriver, time::embassy_monotonic_mailbox_clock};

/// Builds an Embassy actor-system configuration with Embassy-backed dispatchers and clocks.
#[must_use]
pub fn embassy_actor_system_config<const N: usize>(executor_factory: &EmbassyExecutorFactory<N>) -> ActorSystemConfig {
  let default_settings = DispatcherConfig::with_defaults(DEFAULT_DISPATCHER_ID);
  let default_executor = executor_factory.create(DEFAULT_DISPATCHER_ID);
  let default_configurator: Box<dyn MessageDispatcherFactory> =
    Box::new(DefaultDispatcherFactory::new(&default_settings, default_executor));

  let blocking_settings = DispatcherConfig::with_defaults(DEFAULT_BLOCKING_DISPATCHER_ID);
  let blocking_executor = executor_factory.create(DEFAULT_BLOCKING_DISPATCHER_ID);
  let blocking_configurator: Box<dyn MessageDispatcherFactory> =
    Box::new(DefaultDispatcherFactory::new(&blocking_settings, blocking_executor));

  ActorSystemConfig::new(EmbassyTickDriver::default())
    .with_mailbox_clock(embassy_monotonic_mailbox_clock())
    .with_dispatcher_factory(DEFAULT_DISPATCHER_ID, ArcShared::new(default_configurator))
    .with_dispatcher_factory(DEFAULT_BLOCKING_DISPATCHER_ID, ArcShared::new(blocking_configurator))
}
