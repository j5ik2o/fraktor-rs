use alloc::boxed::Box;
use core::time::Duration;

use fraktor_utils_rs::core::sync::ArcShared;

use super::{DEFAULT_BLOCKING_DISPATCHER_ID, DEFAULT_DISPATCHER_ID, Dispatchers, DispatchersError};
use crate::core::kernel::dispatch::dispatcher_new::{
  DefaultDispatcherConfigurator, DispatcherSettings, ExecuteError, Executor, ExecutorShared,
  MessageDispatcherConfigurator,
};

struct NoopExecutor;

impl Executor for NoopExecutor {
  fn execute(&mut self, _task: Box<dyn FnOnce() + Send + 'static>) -> Result<(), ExecuteError> {
    Ok(())
  }

  fn shutdown(&mut self) {}
}

fn make_default_configurator(id: &str) -> ArcShared<Box<dyn MessageDispatcherConfigurator>> {
  let settings = DispatcherSettings::with_defaults(id).with_shutdown_timeout(Duration::from_secs(2));
  let executor = ExecutorShared::new(NoopExecutor);
  let configurator: Box<dyn MessageDispatcherConfigurator> =
    Box::new(DefaultDispatcherConfigurator::new(&settings, executor));
  ArcShared::new(configurator)
}

#[test]
fn register_then_resolve_returns_same_dispatcher() {
  let mut dispatchers = Dispatchers::new();
  let configurator = make_default_configurator("default");
  dispatchers.register("default", configurator).expect("register");
  let shared = dispatchers.resolve("default").expect("resolve");
  assert_eq!(shared.id(), "default");
}

#[test]
fn duplicate_register_returns_error() {
  let mut dispatchers = Dispatchers::new();
  dispatchers.register("dup", make_default_configurator("dup")).expect("first");
  match dispatchers.register("dup", make_default_configurator("dup")) {
    | Ok(()) => panic!("expected duplicate error"),
    | Err(err) => assert!(matches!(err, DispatchersError::Duplicate(_))),
  }
}

#[test]
fn unknown_resolve_returns_error() {
  let dispatchers = Dispatchers::new();
  match dispatchers.resolve("missing") {
    | Ok(_) => panic!("expected unknown id error"),
    | Err(err) => assert!(matches!(err, DispatchersError::Unknown(_))),
  }
}

#[test]
fn pekko_compat_id_normalises_to_default() {
  let mut dispatchers = Dispatchers::new();
  dispatchers.register(DEFAULT_DISPATCHER_ID, make_default_configurator("default")).expect("register default");
  let resolved = dispatchers.resolve("pekko.actor.default-dispatcher").expect("resolve compat id");
  assert_eq!(resolved.id(), "default");
}

#[test]
fn pekko_internal_dispatcher_id_normalises_to_default() {
  let mut dispatchers = Dispatchers::new();
  dispatchers.register(DEFAULT_DISPATCHER_ID, make_default_configurator("default")).expect("register default");
  let resolved = dispatchers.resolve("pekko.actor.internal-dispatcher").expect("resolve internal");
  assert_eq!(resolved.id(), "default");
}

#[test]
fn ensure_default_inserts_when_missing() {
  let mut dispatchers = Dispatchers::new();
  dispatchers.ensure_default(|| make_default_configurator("default"));
  let resolved = dispatchers.resolve(DEFAULT_DISPATCHER_ID).expect("resolve default");
  assert_eq!(resolved.id(), "default");
  let blocking = dispatchers.resolve(DEFAULT_BLOCKING_DISPATCHER_ID).expect("resolve blocking");
  assert_eq!(blocking.id(), "default");
}

#[test]
fn ensure_default_is_idempotent_when_present() {
  let mut dispatchers = Dispatchers::new();
  dispatchers.register(DEFAULT_DISPATCHER_ID, make_default_configurator("first")).expect("register");
  dispatchers.ensure_default(|| make_default_configurator("second"));
  // The original configurator stays.
  let resolved = dispatchers.resolve(DEFAULT_DISPATCHER_ID).expect("resolve default");
  assert_eq!(resolved.id(), "first");
}
