use alloc::{boxed::Box, string::ToString};
use core::{num::NonZeroUsize, time::Duration};

use fraktor_utils_core_rs::core::sync::ArcShared;

use super::PinnedDispatcher;
use crate::core::kernel::{
  actor::{
    Actor, ActorCell, ActorContext, error::ActorError, messaging::AnyMessageView, props::Props, spawn::SpawnError,
  },
  dispatch::dispatcher::{DispatcherSettings, ExecuteError, Executor, ExecutorSharedFactory, MessageDispatcher},
  system::{ActorSystem, shared_factory::BuiltinSpinSharedFactory},
};

struct ProbeActor;

impl Actor for ProbeActor {
  fn receive(&mut self, _ctx: &mut ActorContext<'_>, _message: AnyMessageView<'_>) -> Result<(), ActorError> {
    Ok(())
  }
}

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

fn make_dispatcher() -> PinnedDispatcher {
  // Note the throughput value here is intentionally small; PinnedDispatcher must
  // override it to usize::MAX so we can verify the normalisation.
  let settings = DispatcherSettings::new("pinned-id", nz(3), Some(Duration::from_millis(50)), Duration::from_secs(1));
  let executor = ExecutorSharedFactory::create(&BuiltinSpinSharedFactory::new(), Box::new(NoopExecutor));
  PinnedDispatcher::new(&settings, executor)
}

fn make_actor_cell(name: &str) -> (ActorSystem, ArcShared<ActorCell>) {
  let system = ActorSystem::new_empty();
  let state = system.state();
  let props = Props::from_fn(|| ProbeActor);
  let pid = state.allocate_pid();
  let cell = ActorCell::create(state.clone(), pid, None, name.to_string(), &props).expect("create actor cell");
  state.register_cell(cell.clone());
  (system, cell)
}

fn make_two_actor_cells_same_system() -> (ActorSystem, ArcShared<ActorCell>, ArcShared<ActorCell>) {
  let system = ActorSystem::new_empty();
  let state = system.state();
  let props = Props::from_fn(|| ProbeActor);
  let pid_a = state.allocate_pid();
  let cell_a =
    ActorCell::create(state.clone(), pid_a, None, "pinned-a".to_string(), &props).expect("create actor cell a");
  state.register_cell(cell_a.clone());
  let pid_b = state.allocate_pid();
  let cell_b =
    ActorCell::create(state.clone(), pid_b, None, "pinned-b".to_string(), &props).expect("create actor cell b");
  state.register_cell(cell_b.clone());
  (system, cell_a, cell_b)
}

#[test]
fn new_normalises_throughput_and_deadline() {
  let dispatcher = make_dispatcher();
  assert_eq!(dispatcher.throughput(), NonZeroUsize::new(usize::MAX).expect("non-zero"));
  assert_eq!(dispatcher.throughput_deadline(), None);
  assert_eq!(dispatcher.id(), "pinned-id");
  assert_eq!(dispatcher.shutdown_timeout(), Duration::from_secs(1));
}

#[test]
fn register_actor_sets_owner_and_increments_inhabitants() {
  let mut dispatcher = make_dispatcher();
  let (_system, cell) = make_actor_cell("pinned-1");
  dispatcher.register_actor(&cell).expect("register first actor");
  assert_eq!(dispatcher.owner(), Some(cell.pid()));
  assert_eq!(dispatcher.inhabitants(), 1);
}

#[test]
fn register_actor_rejects_second_owner() {
  let mut dispatcher = make_dispatcher();
  let (_system, cell_a, cell_b) = make_two_actor_cells_same_system();
  assert_ne!(cell_a.pid(), cell_b.pid());
  dispatcher.register_actor(&cell_a).expect("register first actor");
  let err = dispatcher.register_actor(&cell_b).expect_err("second owner should be rejected");
  assert!(matches!(err, SpawnError::DispatcherAlreadyOwned));
}

#[test]
fn register_actor_allows_same_actor_to_reattach() {
  let mut dispatcher = make_dispatcher();
  let (_system, cell) = make_actor_cell("pinned-reattach");
  dispatcher.register_actor(&cell).expect("first attach");
  dispatcher.register_actor(&cell).expect("re-attach");
  assert_eq!(dispatcher.inhabitants(), 2);
}

#[test]
fn unregister_actor_clears_owner_after_detach() {
  let mut dispatcher = make_dispatcher();
  let (_system, cell) = make_actor_cell("pinned-detach");
  dispatcher.register_actor(&cell).expect("register");
  dispatcher.unregister_actor(&cell);
  assert_eq!(dispatcher.owner(), None);
  assert_eq!(dispatcher.inhabitants(), 0);
}

#[test]
fn detach_then_new_owner_can_register() {
  let mut dispatcher = make_dispatcher();
  let (_system, cell_a, cell_b) = make_two_actor_cells_same_system();
  dispatcher.register_actor(&cell_a).expect("first owner");
  dispatcher.unregister_actor(&cell_a);
  dispatcher.register_actor(&cell_b).expect("second owner accepted after detach");
  assert_eq!(dispatcher.owner(), Some(cell_b.pid()));
}
