use alloc::boxed::Box;
use core::{num::NonZeroUsize, time::Duration};

use super::DefaultDispatcher;
use crate::core::kernel::dispatch::dispatcher_new::{
  DispatcherSettings, ExecuteError, Executor, ExecutorShared, MessageDispatcher,
};

struct NoopExecutor;

impl Executor for NoopExecutor {
  fn execute(&mut self, _task: Box<dyn FnOnce() + Send + 'static>) -> Result<(), ExecuteError> {
    Ok(())
  }

  fn shutdown(&mut self) {}
}

fn nz(value: usize) -> NonZeroUsize {
  NonZeroUsize::new(value).expect("non-zero")
}

fn make_dispatcher() -> DefaultDispatcher {
  let settings = DispatcherSettings::new("default-id", nz(7), None, Duration::from_secs(1));
  let executor = ExecutorShared::new(NoopExecutor);
  DefaultDispatcher::new(&settings, executor)
}

#[test]
fn new_initialises_core_from_settings() {
  let dispatcher = make_dispatcher();
  assert_eq!(dispatcher.id(), "default-id");
  assert_eq!(dispatcher.throughput(), nz(7));
  assert_eq!(dispatcher.shutdown_timeout(), Duration::from_secs(1));
  assert_eq!(dispatcher.inhabitants(), 0);
}

#[test]
fn default_register_unregister_updates_inhabitants() {
  let mut dispatcher = make_dispatcher();
  // We don't yet have an ActorCell builder available; the inhabitants counter
  // is exercised through the trait default impl by going via core_mut.
  dispatcher.core_mut().mark_attach();
  dispatcher.core_mut().mark_attach();
  assert_eq!(dispatcher.inhabitants(), 2);
  dispatcher.core_mut().mark_detach();
  assert_eq!(dispatcher.inhabitants(), 1);
}

#[test]
fn shutdown_resets_state() {
  let mut dispatcher = make_dispatcher();
  dispatcher.core_mut().mark_attach();
  dispatcher.core_mut().mark_detach();
  let _ = dispatcher.core_mut().schedule_shutdown_if_sensible();
  dispatcher.shutdown();
  assert_eq!(dispatcher.inhabitants(), 0);
}
