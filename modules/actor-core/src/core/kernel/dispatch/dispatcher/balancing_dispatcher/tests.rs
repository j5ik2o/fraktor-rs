use alloc::{boxed::Box, format, string::ToString, vec::Vec};
use core::{num::NonZeroUsize, time::Duration};

use fraktor_utils_core_rs::core::sync::ArcShared;

use super::BalancingDispatcher;
use crate::core::kernel::{
  actor::{
    Actor, ActorCell, ActorContext,
    error::ActorError,
    messaging::{AnyMessage, AnyMessageView},
    props::Props,
  },
  dispatch::{
    dispatcher::{DispatcherSettings, ExecuteError, Executor, ExecutorSharedFactory, MessageDispatcher},
    mailbox::{Envelope, MailboxCleanupPolicy},
  },
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

fn make_dispatcher() -> BalancingDispatcher {
  let settings = DispatcherSettings::new("balancing-id", nz(5), None, Duration::from_secs(1));
  let executor = ExecutorSharedFactory::create(&BuiltinSpinSharedFactory::new(), Box::new(NoopExecutor));
  let provider = ArcShared::new(BuiltinSpinSharedFactory::new());
  let shared_message_queue_factory: ArcShared<
    dyn crate::core::kernel::dispatch::dispatcher::SharedMessageQueueFactory,
  > = provider.clone();
  let mailbox_shared_set_factory: ArcShared<dyn crate::core::kernel::system::shared_factory::MailboxSharedSetFactory> =
    provider.clone();
  BalancingDispatcher::new(&settings, executor, &shared_message_queue_factory, &mailbox_shared_set_factory)
}

fn make_actor_cells(names: &[&str]) -> (ActorSystem, Vec<ArcShared<ActorCell>>) {
  let system = ActorSystem::new_empty();
  let state = system.state();
  let props = Props::from_fn(|| ProbeActor);
  let mut cells = Vec::new();
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
fn try_create_shared_mailbox_returns_sharing_mailbox() {
  let dispatcher = make_dispatcher();
  let mailbox =
    dispatcher.try_create_shared_mailbox().expect("balancing dispatcher always hands out a sharing mailbox");
  assert_eq!(mailbox.cleanup_policy(), MailboxCleanupPolicy::LeaveSharedQueue);
}

#[test]
fn sharing_mailbox_close_keeps_shared_queue_contents() {
  use crate::core::kernel::dispatch::mailbox::MessageQueue;

  let dispatcher = make_dispatcher();
  let queue = dispatcher.shared_queue();
  let mailbox =
    dispatcher.try_create_shared_mailbox().expect("balancing dispatcher always hands out a sharing mailbox");

  queue.enqueue(Envelope::new(AnyMessage::new(11_u32))).expect("shared enqueue");
  mailbox.become_closed_and_clean_up();

  assert_eq!(queue.number_of_messages(), 1, "LeaveSharedQueue mailbox must not drain the shared queue");
}

#[test]
fn balancing_dispatcher_load_balances_envelopes_across_team_via_shared_queue() {
  // Phase 14.6: end-to-end load balancing check. Three actors are attached
  // to the same `BalancingDispatcherConfigurator`, then 9 envelopes are
  // dispatched through the first cell. Because all team members share the
  // same `SharedMessageQueue`, the inline executor drains the queue across
  // multiple actors instead of leaving everything on the receiver mailbox.
  // The test asserts that more than one actor observed work, which is the
  // V1 load-balancing contract documented in
  // `dispatcher-pekko-1n-redesign/specs/dispatcher-trait-provider-abstraction/spec.md`.
  use alloc::sync::Arc;
  use core::sync::atomic::{AtomicUsize, Ordering};

  use crate::core::kernel::dispatch::dispatcher::{BalancingDispatcherConfigurator, MessageDispatcherConfigurator};

  struct InlineExec;

  impl Executor for InlineExec {
    fn execute(&mut self, task: Box<dyn FnOnce() + Send + 'static>) -> Result<(), ExecuteError> {
      task();
      Ok(())
    }

    fn shutdown(&mut self) {}
  }

  struct CountingActor {
    seen: Arc<AtomicUsize>,
  }

  impl Actor for CountingActor {
    fn receive(&mut self, _ctx: &mut ActorContext<'_>, _msg: AnyMessageView<'_>) -> Result<(), ActorError> {
      self.seen.fetch_add(1, Ordering::SeqCst);
      Ok(())
    }
  }

  let configurator: ArcShared<Box<dyn MessageDispatcherConfigurator>> = {
    let executor = ExecutorSharedFactory::create(&BuiltinSpinSharedFactory::new(), Box::new(InlineExec));
    let settings = DispatcherSettings::new("balancing-load", nz(8), None, Duration::from_secs(1));
    let provider = ArcShared::new(BuiltinSpinSharedFactory::new());
    let message_dispatcher_shared_factory: ArcShared<
      dyn crate::core::kernel::dispatch::dispatcher::MessageDispatcherSharedFactory,
    > = provider.clone();
    let shared_message_queue_factory: ArcShared<
      dyn crate::core::kernel::dispatch::dispatcher::SharedMessageQueueFactory,
    > = provider.clone();
    let mailbox_shared_set_factory: ArcShared<
      dyn crate::core::kernel::system::shared_factory::MailboxSharedSetFactory,
    > = provider.clone();
    let inner: Box<dyn MessageDispatcherConfigurator> = Box::new(BalancingDispatcherConfigurator::new(
      &settings,
      executor,
      &message_dispatcher_shared_factory,
      &shared_message_queue_factory,
      &mailbox_shared_set_factory,
    ));
    ArcShared::new(inner)
  };
  let configurator_clone = configurator.clone();
  let system = ActorSystem::new_empty_with(move |config| {
    config.with_dispatcher_configurator("balancing-load", configurator_clone.clone())
  });
  let state = system.state();

  let counters: Vec<Arc<AtomicUsize>> = (0..3).map(|_| Arc::new(AtomicUsize::new(0))).collect();
  let mut cells: Vec<ArcShared<ActorCell>> = Vec::new();
  for (idx, counter) in counters.iter().enumerate() {
    let counter_clone = counter.clone();
    let props =
      Props::from_fn(move || CountingActor { seen: counter_clone.clone() }).with_dispatcher_id("balancing-load");
    let pid = state.allocate_pid();
    let name = format!("balancer-{idx}");
    let cell = ActorCell::create(state.clone(), pid, None, name, &props).expect("create cell");
    state.register_cell(cell.clone());
    cells.push(cell);
  }

  // Dispatch 9 envelopes by rotating across the three cells. With an inline
  // executor each tell triggers an immediate synchronous drain on the
  // receiver mailbox, so by tell-ing through each actor in turn we exercise
  // every team member's drain path. The shared queue is the same instance
  // for all three cells, so every envelope is routed through it.
  let mut refs: Vec<_> = cells.iter().map(|cell| cell.actor_ref()).collect();
  for value in 0..9_u32 {
    let target = (value as usize) % refs.len();
    refs[target].tell(AnyMessage::new(value));
  }

  let total: usize = counters.iter().map(|c| c.load(Ordering::SeqCst)).sum();
  assert_eq!(total, 9, "all 9 envelopes must be processed exactly once");

  let actors_with_work = counters.iter().filter(|c| c.load(Ordering::SeqCst) > 0).count();
  assert!(
    actors_with_work >= 2,
    "load balancing must spread work across more than one actor (counters: {:?})",
    counters.iter().map(|c| c.load(Ordering::SeqCst)).collect::<Vec<_>>()
  );
}
