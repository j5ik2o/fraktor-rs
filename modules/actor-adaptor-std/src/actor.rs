//! Actor-specific helpers that require the standard library.

mod panic_invoke_guard;
mod panic_invoke_guard_factory;

#[cfg(feature = "tokio-executor")]
use alloc::boxed::Box;

use fraktor_actor_core_kernel_rs::actor::setup::ActorSystemConfig;
#[cfg(feature = "tokio-executor")]
use fraktor_actor_core_kernel_rs::dispatch::dispatcher::{
  DEFAULT_BLOCKING_DISPATCHER_ID, DEFAULT_DISPATCHER_ID, DefaultDispatcherFactory, DispatcherConfig, ExecutorFactory,
  MessageDispatcherFactory,
};
#[cfg(feature = "tokio-executor")]
use fraktor_utils_core_rs::sync::ArcShared;
pub use panic_invoke_guard::PanicInvokeGuard;
pub use panic_invoke_guard_factory::PanicInvokeGuardFactory;
#[cfg(feature = "tokio-executor")]
use tokio::runtime::Handle;

#[cfg(feature = "tokio-executor")]
use crate::{
  dispatch::dispatcher::{TokioExecutorFactory, TokioTaskExecutorFactory},
  tick_driver::TokioTickDriver,
  time::std_monotonic_mailbox_clock,
};

/// Installs the std panic guard into an actor-system configuration.
#[must_use]
pub fn install_panic_invoke_guard(config: ActorSystemConfig) -> ActorSystemConfig {
  config.with_invoke_guard_factory(PanicInvokeGuardFactory::shared())
}

/// Builds a std Tokio actor-system configuration with separated default and blocking dispatchers.
///
/// The default dispatcher uses [`TokioTaskExecutorFactory`] so actor mailbox work
/// is scheduled as Tokio tasks. The default blocking dispatcher uses the
/// existing [`TokioExecutorFactory`], preserving its `spawn_blocking` semantics
/// for callers that opt into [`DEFAULT_BLOCKING_DISPATCHER_ID`].
///
/// Use this helper for actors that should run short, non-blocking handlers on a
/// Tokio task executor. Put synchronous file I/O, CPU-heavy work, or legacy
/// blocking calls on props configured with [`DEFAULT_BLOCKING_DISPATCHER_ID`].
/// The supplied [`Handle`] must outlive the actor system.
#[cfg(feature = "tokio-executor")]
#[must_use]
pub fn tokio_actor_system_config(handle: Handle) -> ActorSystemConfig {
  let default_settings = DispatcherConfig::with_defaults(DEFAULT_DISPATCHER_ID);
  let default_executor = TokioTaskExecutorFactory::new(handle.clone()).create(DEFAULT_DISPATCHER_ID);
  let default_configurator: Box<dyn MessageDispatcherFactory> =
    Box::new(DefaultDispatcherFactory::new(&default_settings, default_executor));

  let blocking_settings = DispatcherConfig::with_defaults(DEFAULT_BLOCKING_DISPATCHER_ID);
  let blocking_executor = TokioExecutorFactory::new(handle).create(DEFAULT_BLOCKING_DISPATCHER_ID);
  let blocking_configurator: Box<dyn MessageDispatcherFactory> =
    Box::new(DefaultDispatcherFactory::new(&blocking_settings, blocking_executor));

  ActorSystemConfig::new(TokioTickDriver::default())
    .with_mailbox_clock(std_monotonic_mailbox_clock())
    .with_dispatcher_factory(DEFAULT_DISPATCHER_ID, ArcShared::new(default_configurator))
    .with_dispatcher_factory(DEFAULT_BLOCKING_DISPATCHER_ID, ArcShared::new(blocking_configurator))
}
