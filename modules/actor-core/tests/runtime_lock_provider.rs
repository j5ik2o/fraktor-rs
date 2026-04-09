#![cfg(not(target_os = "none"))]

use std::{
  panic::{AssertUnwindSafe, catch_unwind},
  sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicUsize, Ordering},
  },
};

use fraktor_actor_core_rs::core::kernel::{
  actor::{
    Actor, ActorCell, ActorContext, Pid,
    actor_ref::{ActorRef, ActorRefSender, ActorRefSenderShared, SendOutcome},
    error::{ActorError, SendError},
    messaging::{AnyMessage, AnyMessageView},
    props::Props,
  },
  runtime_lock_provider::{
    ActorRuntimeLockProvider, BuiltinSpinRuntimeLockProvider, DispatcherLockCell, ExecutorLockCell, MailboxLockSet,
    SenderLockCell,
  },
  system::ActorSystem,
};
use fraktor_utils_adaptor_std_rs::{new_debug_runtime_lock_provider, new_std_runtime_lock_provider};
use fraktor_utils_core_rs::core::sync::ArcShared;

struct SilentActor;

impl Actor for SilentActor {
  fn receive(&mut self, _ctx: &mut ActorContext<'_>, _message: AnyMessageView<'_>) -> Result<(), ActorError> {
    Ok(())
  }
}

#[derive(Default)]
struct ProviderCounters {
  dispatcher_cells: AtomicUsize,
  executor_cells:   AtomicUsize,
  sender_cells:     AtomicUsize,
  mailbox_sets:     AtomicUsize,
}

struct CountingRuntimeLockProvider {
  counters: Arc<ProviderCounters>,
}

impl CountingRuntimeLockProvider {
  fn shared(counters: Arc<ProviderCounters>) -> ArcShared<dyn ActorRuntimeLockProvider> {
    let provider: ArcShared<dyn ActorRuntimeLockProvider> = ArcShared::new(Self { counters });
    provider
  }
}

impl ActorRuntimeLockProvider for CountingRuntimeLockProvider {
  fn new_dispatcher_cell(
    &self,
    dispatcher: Box<dyn fraktor_actor_core_rs::core::kernel::dispatch::dispatcher::MessageDispatcher>,
  ) -> DispatcherLockCell {
    self.counters.dispatcher_cells.fetch_add(1, Ordering::SeqCst);
    BuiltinSpinRuntimeLockProvider::default().new_dispatcher_cell(dispatcher)
  }

  fn new_executor_cell(
    &self,
    executor: Box<dyn fraktor_actor_core_rs::core::kernel::dispatch::dispatcher::Executor>,
  ) -> ExecutorLockCell {
    self.counters.executor_cells.fetch_add(1, Ordering::SeqCst);
    BuiltinSpinRuntimeLockProvider::default().new_executor_cell(executor)
  }

  fn new_sender_cell(&self, sender: Box<dyn ActorRefSender>) -> SenderLockCell {
    self.counters.sender_cells.fetch_add(1, Ordering::SeqCst);
    BuiltinSpinRuntimeLockProvider::default().new_sender_cell(sender)
  }

  fn new_mailbox_lock_set(&self) -> MailboxLockSet {
    self.counters.mailbox_sets.fetch_add(1, Ordering::SeqCst);
    BuiltinSpinRuntimeLockProvider::default().new_mailbox_lock_set()
  }
}

struct RecursiveSender {
  recursion_started: Arc<AtomicBool>,
  self_ref:          Arc<Mutex<Option<ActorRef>>>,
}

impl ActorRefSender for RecursiveSender {
  fn send(&mut self, _message: AnyMessage) -> Result<SendOutcome, SendError> {
    if !self.recursion_started.swap(true, Ordering::SeqCst)
      && let Some(mut actor_ref) = self.self_ref.lock().expect("recursive sender self ref mutex").clone()
    {
      actor_ref.tell(AnyMessage::new("recursive"));
    }
    Ok(SendOutcome::Delivered)
  }
}

fn create_probe_cell(system: &ActorSystem, name: &str) {
  let state = system.state();
  let pid = state.allocate_pid();
  let props = Props::from_fn(|| SilentActor);
  let cell = ActorCell::create(state.clone(), pid, None, name.to_string(), &props).expect("probe cell");
  state.register_cell(cell);
}

#[test]
fn debug_provider_panics_on_same_thread_reentrant_tell() {
  let system = ActorSystem::new_empty();
  let self_ref = Arc::new(Mutex::new(None::<ActorRef>));
  let sender =
    RecursiveSender { recursion_started: Arc::new(AtomicBool::new(false)), self_ref: self_ref.clone() };
  let sender = ActorRefSenderShared::new_with_provider(sender, new_debug_runtime_lock_provider());
  let mut actor_ref = ActorRef::from_shared(Pid::new(1, 0), sender, &system.state());
  *self_ref.lock().expect("recursive self ref mutex") = Some(actor_ref.clone());

  let result = catch_unwind(AssertUnwindSafe(|| {
    actor_ref.tell(AnyMessage::new("start"));
  }));

  assert!(result.is_err(), "debug runtime lock provider should panic on reentrant tell");
}

#[test]
fn default_spin_fallback_and_system_scoped_override_both_work() {
  let default_system = ActorSystem::new_empty();
  create_probe_cell(&default_system, "default-provider-cell");

  let counters = Arc::new(ProviderCounters::default());
  let provider = CountingRuntimeLockProvider::shared(counters.clone());
  let override_system = ActorSystem::new_empty_with(|config| config.with_runtime_lock_provider(provider));
  create_probe_cell(&override_system, "override-provider-cell");

  assert!(counters.dispatcher_cells.load(Ordering::SeqCst) >= 1);
  assert!(counters.executor_cells.load(Ordering::SeqCst) >= 1);
  assert!(counters.sender_cells.load(Ordering::SeqCst) >= 1);
  assert!(counters.mailbox_sets.load(Ordering::SeqCst) >= 1);
}

#[test]
fn distinct_actor_systems_keep_runtime_lock_provider_families_isolated() {
  let counters_a = Arc::new(ProviderCounters::default());
  let provider_a = CountingRuntimeLockProvider::shared(counters_a.clone());
  let system_a = ActorSystem::new_empty_with(|config| config.with_runtime_lock_provider(provider_a));
  create_probe_cell(&system_a, "provider-a-cell");

  let dispatcher_a = counters_a.dispatcher_cells.load(Ordering::SeqCst);
  let executor_a = counters_a.executor_cells.load(Ordering::SeqCst);
  let sender_a = counters_a.sender_cells.load(Ordering::SeqCst);
  let mailbox_a = counters_a.mailbox_sets.load(Ordering::SeqCst);
  assert!(dispatcher_a >= 1 && executor_a >= 1 && sender_a >= 1 && mailbox_a >= 1);

  let counters_b = Arc::new(ProviderCounters::default());
  let provider_b = CountingRuntimeLockProvider::shared(counters_b.clone());
  let system_b = ActorSystem::new_empty_with(|config| config.with_runtime_lock_provider(provider_b));
  create_probe_cell(&system_b, "provider-b-cell");

  assert_eq!(counters_a.dispatcher_cells.load(Ordering::SeqCst), dispatcher_a);
  assert_eq!(counters_a.executor_cells.load(Ordering::SeqCst), executor_a);
  assert_eq!(counters_a.sender_cells.load(Ordering::SeqCst), sender_a);
  assert_eq!(counters_a.mailbox_sets.load(Ordering::SeqCst), mailbox_a);

  assert!(counters_b.dispatcher_cells.load(Ordering::SeqCst) >= 1);
  assert!(counters_b.executor_cells.load(Ordering::SeqCst) >= 1);
  assert!(counters_b.sender_cells.load(Ordering::SeqCst) >= 1);
  assert!(counters_b.mailbox_sets.load(Ordering::SeqCst) >= 1);
}

#[test]
fn std_runtime_lock_provider_can_be_selected_explicitly() {
  let system = ActorSystem::new_empty_with(|config| config.with_runtime_lock_provider(new_std_runtime_lock_provider()));
  create_probe_cell(&system, "std-provider-cell");
}
