use alloc::{boxed::Box, string::ToString};
use core::{num::NonZeroUsize, time::Duration};

use fraktor_utils_rs::core::sync::ArcShared;

use super::BalancingDispatcher;
use crate::core::kernel::{
  actor::{
    Actor, ActorCell, ActorContext,
    error::ActorError,
    messaging::{AnyMessage, AnyMessageView},
    props::Props,
  },
  dispatch::{
    dispatcher::{DispatcherSettings, ExecuteError, Executor, ExecutorShared, MessageDispatcher},
    mailbox::{Envelope, MailboxCleanupPolicy},
  },
  system::ActorSystem,
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

fn make_dispatcher() -> BalancingDispatcher {
  let settings = DispatcherSettings::new("balancing-id", nz(5), None, Duration::from_secs(1));
  let executor = ExecutorShared::new(NoopExecutor);
  BalancingDispatcher::new(&settings, executor)
}

fn make_actor_cells(names: &[&str]) -> (ActorSystem, alloc::vec::Vec<ArcShared<ActorCell>>) {
  let system = ActorSystem::new_empty();
  let state = system.state();
  let props = Props::from_fn(|| ProbeActor);
  let mut cells = alloc::vec::Vec::new();
  for name in names {
    let pid = state.allocate_pid();
    let cell = ActorCell::create(state.clone(), pid, None, name.to_string(), &props).expect("create actor cell");
    state.register_cell(cell.clone());
    cells.push(cell);
  }
  (system, cells)
}

#[test]
fn shared_queue_is_thread_safe_via_sequential_enqueue() {
  let dispatcher = make_dispatcher();
  let queue = dispatcher.shared_queue();
  use crate::core::kernel::dispatch::mailbox::{Envelope, MessageQueue};
  let _ = queue.enqueue(Envelope::new(AnyMessage::new(1_u32)));
  let _ = queue.enqueue(Envelope::new(AnyMessage::new(2_u32)));
  assert_eq!(queue.number_of_messages(), 2);
  assert!(queue.dequeue().is_some());
  assert!(queue.dequeue().is_some());
  assert!(queue.dequeue().is_none());
}

#[test]
fn register_actor_adds_to_team_and_increments_inhabitants() {
  let mut dispatcher = make_dispatcher();
  let (_system, cells) = make_actor_cells(&["a", "b", "c"]);
  for cell in &cells {
    dispatcher.register_actor(cell).expect("register");
  }
  assert_eq!(dispatcher.inhabitants(), 3);
  assert_eq!(dispatcher.team_size(), 3);
}

#[test]
fn unregister_actor_removes_from_team() {
  let mut dispatcher = make_dispatcher();
  let (_system, cells) = make_actor_cells(&["a", "b"]);
  for cell in &cells {
    dispatcher.register_actor(cell).expect("register");
  }
  dispatcher.unregister_actor(&cells[0]);
  assert_eq!(dispatcher.inhabitants(), 1);
  assert_eq!(dispatcher.team_size(), 1);
}

#[test]
fn dispatch_enqueues_to_shared_queue_and_returns_team_candidates() {
  let mut dispatcher = make_dispatcher();
  let (_system, cells) = make_actor_cells(&["a", "b", "c"]);
  for cell in &cells {
    dispatcher.register_actor(cell).expect("register");
  }
  let envelope = Envelope::new(AnyMessage::new(42_u32));
  let candidates = dispatcher.dispatch(&cells[0], envelope).expect("dispatch");
  // Receiver mailbox is first; remaining team members follow.
  assert_eq!(candidates.len(), 3);
  use crate::core::kernel::dispatch::mailbox::MessageQueue;
  let q = dispatcher.shared_queue();
  assert_eq!(q.number_of_messages(), 1);
}

#[test]
fn create_mailbox_returns_sharing_mailbox() {
  use crate::core::kernel::dispatch::mailbox::{MailboxType, UnboundedMailboxType};
  let dispatcher = make_dispatcher();
  let (_system, cells) = make_actor_cells(&["solo"]);
  let mailbox_type: alloc::boxed::Box<dyn MailboxType> = alloc::boxed::Box::new(UnboundedMailboxType::default());
  let mailbox = dispatcher.create_mailbox(&cells[0], mailbox_type.as_ref());
  assert_eq!(mailbox.cleanup_policy(), MailboxCleanupPolicy::LeaveSharedQueue);
}
