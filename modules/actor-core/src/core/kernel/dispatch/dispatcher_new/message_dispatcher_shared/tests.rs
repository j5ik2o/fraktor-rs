use alloc::{boxed::Box, sync::Arc};
use core::{
  num::NonZeroUsize,
  sync::atomic::{AtomicUsize, Ordering},
  time::Duration,
};

use super::MessageDispatcherShared;
use crate::core::kernel::dispatch::dispatcher_new::{
  DefaultDispatcher, DispatcherSettings, ExecuteError, Executor, ExecutorShared,
};

struct CountingExecutor {
  submits: Arc<AtomicUsize>,
}

impl Executor for CountingExecutor {
  fn execute(&mut self, task: Box<dyn FnOnce() + Send + 'static>) -> Result<(), ExecuteError> {
    self.submits.fetch_add(1, Ordering::SeqCst);
    task();
    Ok(())
  }

  fn shutdown(&mut self) {}
}

fn nz(value: usize) -> NonZeroUsize {
  NonZeroUsize::new(value).expect("non-zero")
}

#[test]
fn shared_query_methods_delegate_to_inner() {
  let executor = ExecutorShared::new(CountingExecutor { submits: Arc::new(AtomicUsize::new(0)) });
  let settings = DispatcherSettings::new("shared", nz(11), Some(Duration::from_millis(7)), Duration::from_secs(2));
  let dispatcher = DefaultDispatcher::new(&settings, executor);
  let shared = MessageDispatcherShared::new(dispatcher);
  assert_eq!(shared.id(), "shared");
  assert_eq!(shared.throughput(), nz(11));
  assert_eq!(shared.throughput_deadline(), Some(Duration::from_millis(7)));
  assert_eq!(shared.shutdown_timeout(), Duration::from_secs(2));
  assert_eq!(shared.inhabitants(), 0);
}

#[test]
fn clone_shares_inner_state() {
  let executor = ExecutorShared::new(CountingExecutor { submits: Arc::new(AtomicUsize::new(0)) });
  let settings = DispatcherSettings::with_defaults("clone-test");
  let dispatcher = DefaultDispatcher::new(&settings, executor);
  let shared = MessageDispatcherShared::new(dispatcher);
  let cloned = shared.clone();
  // Both clones see the same id.
  assert_eq!(shared.id(), cloned.id());
}

#[test]
fn shutdown_invokes_inner_shutdown() {
  let executor = ExecutorShared::new(CountingExecutor { submits: Arc::new(AtomicUsize::new(0)) });
  let settings = DispatcherSettings::with_defaults("shutdown");
  let dispatcher = DefaultDispatcher::new(&settings, executor);
  let shared = MessageDispatcherShared::new(dispatcher);
  shared.shutdown();
}

#[test]
fn dispatch_drives_user_message_through_actor_invoker() {
  use crate::core::kernel::{
    actor::{
      Actor, ActorCell, ActorContext,
      error::ActorError,
      messaging::{AnyMessage, AnyMessageView},
      props::Props,
    },
    dispatch::mailbox::Envelope,
    system::ActorSystem,
  };

  struct CountingActor {
    seen: Arc<AtomicUsize>,
  }

  impl Actor for CountingActor {
    fn receive(&mut self, _ctx: &mut ActorContext<'_>, _message: AnyMessageView<'_>) -> Result<(), ActorError> {
      self.seen.fetch_add(1, Ordering::SeqCst);
      Ok(())
    }
  }

  let system = ActorSystem::new_empty();
  let state = system.state();
  let seen = Arc::new(AtomicUsize::new(0));
  let seen_for_actor = Arc::clone(&seen);
  let props = Props::from_fn(move || CountingActor { seen: seen_for_actor.clone() });
  let pid = state.allocate_pid();
  let cell = ActorCell::create(state.clone(), pid, None, "drive-test".into(), &props).expect("create cell");
  state.register_cell(cell.clone());

  let executor = ExecutorShared::new(CountingExecutor { submits: Arc::new(AtomicUsize::new(0)) });
  let settings = DispatcherSettings::new("dispatch-drive", nz(8), None, Duration::from_secs(1));
  let dispatcher = DefaultDispatcher::new(&settings, executor);
  let shared = MessageDispatcherShared::new(dispatcher);

  shared.dispatch(&cell, Envelope::new(AnyMessage::new(7_u32))).expect("dispatch");
  assert_eq!(seen.load(Ordering::SeqCst), 1, "user message should be drained through invoker");
}

#[test]
fn resolve_new_dispatcher_from_actor_system_returns_registered_configurator() {
  use alloc::boxed::Box;

  use fraktor_utils_rs::core::sync::ArcShared;

  use crate::core::kernel::{
    dispatch::dispatcher_new::{DefaultDispatcherConfigurator, MessageDispatcherConfigurator},
    system::ActorSystem,
  };

  let system = ActorSystem::new_empty_with(|config| {
    let executor = ExecutorShared::new(CountingExecutor { submits: Arc::new(AtomicUsize::new(0)) });
    let settings = DispatcherSettings::new("system-test-dispatch", nz(4), None, Duration::from_secs(1));
    let configurator: Box<dyn MessageDispatcherConfigurator> =
      Box::new(DefaultDispatcherConfigurator::new(&settings, executor));
    let configurator_handle: ArcShared<Box<dyn MessageDispatcherConfigurator>> = ArcShared::new(configurator);
    config.with_new_dispatcher_configurator("system-test-dispatch", configurator_handle)
  });
  let resolved = system.state().resolve_new_dispatcher("system-test-dispatch").expect("registered configurator");
  assert_eq!(resolved.id(), "system-test-dispatch");
}
