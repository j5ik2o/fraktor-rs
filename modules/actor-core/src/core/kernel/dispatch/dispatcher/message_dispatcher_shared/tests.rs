use alloc::{boxed::Box, sync::Arc};
use core::{
  num::NonZeroUsize,
  sync::atomic::{AtomicUsize, Ordering},
  time::Duration,
};
use std::{
  sync::{
    Mutex as StdMutex,
    mpsc::{self, Receiver, Sender},
  },
  thread,
};

use crate::core::kernel::{
  actor::{Actor, ActorContext, error::ActorError, messaging::AnyMessageView},
  dispatch::dispatcher::{
    DefaultDispatcher, DispatcherConfig, ExecuteError, Executor, ExecutorShared, MessageDispatcherShared,
    TrampolineState,
  },
};

struct CountingExecutor {
  submits: Arc<AtomicUsize>,
}

impl Executor for CountingExecutor {
  fn execute(&mut self, task: Box<dyn FnOnce() + Send + 'static>, _affinity_key: u64) -> Result<(), ExecuteError> {
    self.submits.fetch_add(1, Ordering::SeqCst);
    task();
    Ok(())
  }

  fn shutdown(&mut self) {}
}

struct ProbeActor;

impl Actor for ProbeActor {
  fn receive(&mut self, _ctx: &mut ActorContext<'_>, _message: AnyMessageView<'_>) -> Result<(), ActorError> {
    Ok(())
  }
}

struct BlockingActor {
  seen:       Arc<AtomicUsize>,
  started_tx: Sender<()>,
  resume_rx:  Arc<StdMutex<Receiver<()>>>,
}

impl Actor for BlockingActor {
  fn receive(&mut self, _ctx: &mut ActorContext<'_>, _message: AnyMessageView<'_>) -> Result<(), ActorError> {
    let previous = self.seen.fetch_add(1, Ordering::SeqCst);
    if previous == 0 {
      self.started_tx.send(()).expect("blocking actor should signal first receive");
      self.resume_rx.lock().expect("resume lock").recv().expect("resume signal");
    }
    Ok(())
  }
}

fn nz(value: usize) -> NonZeroUsize {
  NonZeroUsize::new(value).expect("non-zero")
}

#[test]
fn shared_query_methods_delegate_to_inner() {
  let executor =
    ExecutorShared::new(Box::new(CountingExecutor { submits: Arc::new(AtomicUsize::new(0)) }), TrampolineState::new());
  let settings = DispatcherConfig::new("shared", nz(11), Some(Duration::from_millis(7)), Duration::from_secs(2));
  let dispatcher = DefaultDispatcher::new(&settings, executor);
  let shared = MessageDispatcherShared::new(Box::new(dispatcher));
  assert_eq!(shared.id(), "shared");
  assert_eq!(shared.throughput(), nz(11));
  assert_eq!(shared.throughput_deadline(), Some(Duration::from_millis(7)));
  assert_eq!(shared.shutdown_timeout(), Duration::from_secs(2));
  assert_eq!(shared.inhabitants(), 0);
}

#[test]
fn clone_shares_inner_state() {
  let executor =
    ExecutorShared::new(Box::new(CountingExecutor { submits: Arc::new(AtomicUsize::new(0)) }), TrampolineState::new());
  let settings = DispatcherConfig::with_defaults("clone-test");
  let dispatcher = DefaultDispatcher::new(&settings, executor);
  let shared = MessageDispatcherShared::new(Box::new(dispatcher));
  let cloned = shared.clone();
  // Both clones see the same id.
  assert_eq!(shared.id(), cloned.id());
}

#[test]
fn shutdown_invokes_inner_shutdown() {
  let executor =
    ExecutorShared::new(Box::new(CountingExecutor { submits: Arc::new(AtomicUsize::new(0)) }), TrampolineState::new());
  let settings = DispatcherConfig::with_defaults("shutdown");
  let dispatcher = DefaultDispatcher::new(&settings, executor);
  let shared = MessageDispatcherShared::new(Box::new(dispatcher));
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

  let executor =
    ExecutorShared::new(Box::new(CountingExecutor { submits: Arc::new(AtomicUsize::new(0)) }), TrampolineState::new());
  let settings = DispatcherConfig::new("dispatch-drive", nz(8), None, Duration::from_secs(1));
  let dispatcher = DefaultDispatcher::new(&settings, executor);
  let shared = MessageDispatcherShared::new(Box::new(dispatcher));

  shared.dispatch(&cell, Envelope::new(AnyMessage::new(7_u32))).expect("dispatch");
  assert_eq!(seen.load(Ordering::SeqCst), 1, "user message should be drained through invoker");
}

#[test]
fn resolve_dispatcher_from_actor_system_returns_registered_configurator() {
  use fraktor_utils_core_rs::core::sync::ArcShared;

  use crate::core::kernel::{
    dispatch::dispatcher::{DefaultDispatcherConfigurator, MessageDispatcherConfigurator},
    system::ActorSystem,
  };

  let system = ActorSystem::new_empty_with(|config| {
    let executor = ExecutorShared::new(
      Box::new(CountingExecutor { submits: Arc::new(AtomicUsize::new(0)) }),
      TrampolineState::new(),
    );
    let settings = DispatcherConfig::new("system-test-dispatch", nz(4), None, Duration::from_secs(1));
    let configurator: Box<dyn MessageDispatcherConfigurator> =
      Box::new(DefaultDispatcherConfigurator::new(&settings, executor));
    let configurator_handle: ArcShared<Box<dyn MessageDispatcherConfigurator>> = ArcShared::new(configurator);
    config.with_dispatcher_configurator("system-test-dispatch", configurator_handle)
  });
  let resolved = system.state().resolve_dispatcher("system-test-dispatch").expect("registered configurator");
  assert_eq!(resolved.id(), "system-test-dispatch");
}

#[test]
fn detach_idle_mailbox_cleans_up_immediately() {
  use crate::core::kernel::{
    actor::{ActorCell, Pid, messaging::AnyMessage, props::Props},
    system::ActorSystem,
  };

  let system = ActorSystem::new_empty();
  let state = system.state();
  let props = Props::from_fn(|| ProbeActor);
  let cell = ActorCell::create(state.clone(), Pid::new(700, 0), None, "idle-detach".into(), &props).expect("create");
  state.register_cell(cell.clone());
  cell.mailbox().enqueue_user(AnyMessage::new("queued")).expect("queued");

  let _schedule = cell.new_dispatcher_shared().detach(&cell);

  assert!(cell.mailbox().is_closed());
  assert_eq!(cell.mailbox().user_len(), 0, "idle detach should clean user queue immediately");
}

#[test]
fn detach_running_mailbox_returns_before_runner_finalizes() {
  use crate::core::kernel::{
    actor::{ActorCell, Pid, messaging::AnyMessage, props::Props},
    system::ActorSystem,
  };

  let system = ActorSystem::new_empty();
  let state = system.state();
  let seen = Arc::new(AtomicUsize::new(0));
  let (started_tx, started_rx) = mpsc::channel();
  let (resume_tx, resume_rx) = mpsc::channel();
  let resume_rx = Arc::new(StdMutex::new(resume_rx));
  let props = Props::from_fn({
    let seen = seen.clone();
    let resume_rx = resume_rx.clone();
    move || BlockingActor { seen: seen.clone(), started_tx: started_tx.clone(), resume_rx: resume_rx.clone() }
  });
  let cell = ActorCell::create(state.clone(), Pid::new(701, 0), None, "running-detach".into(), &props).expect("create");
  state.register_cell(cell.clone());
  cell.mailbox().enqueue_user(AnyMessage::new(1_u32)).expect("first");
  cell.mailbox().enqueue_user(AnyMessage::new(2_u32)).expect("second");

  let mailbox = cell.mailbox();
  let mailbox_for_run = mailbox.clone();
  let run_handle = thread::spawn(move || mailbox_for_run.run(nz(8), None));

  started_rx.recv().expect("runner should start first message");

  let cell_for_detach = cell.clone();
  let (detach_done_tx, detach_done_rx) = mpsc::channel();
  let detach_handle = thread::spawn(move || {
    let schedule = cell_for_detach.new_dispatcher_shared().detach(&cell_for_detach);
    detach_done_tx.send(schedule).expect("detach result");
  });

  detach_done_rx
    .recv_timeout(Duration::from_millis(200))
    .expect("detach should return without waiting for the blocked runner");
  assert!(mailbox.is_closed(), "detach should publish close request immediately");

  resume_tx.send(()).expect("resume");

  assert!(!run_handle.join().expect("runner should complete"));
  detach_handle.join().expect("detach thread should complete");
  assert_eq!(seen.load(Ordering::SeqCst), 1, "runner finalizer must suppress the second queued user message");
  assert_eq!(mailbox.user_len(), 0, "runner finalizer should clean remaining queued user messages");
}
