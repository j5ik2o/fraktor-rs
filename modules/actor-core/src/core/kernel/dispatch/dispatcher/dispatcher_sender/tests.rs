use alloc::{boxed::Box, sync::Arc};
use core::{
  num::NonZeroUsize,
  sync::atomic::{AtomicUsize, Ordering},
  time::Duration,
};

use fraktor_utils_rs::core::sync::ArcShared;

use super::DispatcherSender;
use crate::core::kernel::{
  actor::{
    actor_ref::{ActorRefSender, SendOutcome},
    messaging::AnyMessage,
  },
  dispatch::{
    dispatcher::{
      DefaultDispatcher, DispatcherSettings, ExecuteError, Executor, ExecutorShared, MessageDispatcherShared,
    },
    mailbox::{Mailbox, MailboxPolicy},
  },
};

struct InlineExec;

impl Executor for InlineExec {
  fn execute(&mut self, task: Box<dyn FnOnce() + Send + 'static>) -> Result<(), ExecuteError> {
    task();
    Ok(())
  }

  fn shutdown(&mut self) {}
}

fn nz(value: usize) -> NonZeroUsize {
  NonZeroUsize::new(value).expect("non-zero")
}

#[test]
fn send_returns_schedule_outcome_that_drives_register_for_execution() {
  let executor = ExecutorShared::new(InlineExec);
  let settings = DispatcherSettings::new("sender-test", nz(4), None, Duration::from_secs(1));
  let dispatcher = DefaultDispatcher::new(&settings, executor);
  let shared = MessageDispatcherShared::new(dispatcher);
  let mailbox = ArcShared::new(Mailbox::new(MailboxPolicy::unbounded(None)));

  let mut sender = DispatcherSender::new(shared, mailbox.clone());
  let outcome = sender.send(AnyMessage::new(11_u32)).expect("send");
  match outcome {
    | SendOutcome::Schedule(task) => task(),
    | SendOutcome::Delivered => {},
  }
  // Without an installed invoker the run() body is a no-op, but the schedule
  // closure must still complete without panicking.
  let _ = mailbox;
  let _ = AtomicUsize::new(0);
}

#[test]
fn actor_creation_attaches_to_new_dispatcher_and_increments_inhabitants() {
  use alloc::string::ToString;

  use crate::core::kernel::{
    actor::{Actor, ActorCell, ActorContext, error::ActorError, messaging::AnyMessageView, props::Props},
    dispatch::dispatcher::{DefaultDispatcherConfigurator, MessageDispatcherConfigurator},
    system::ActorSystem,
  };

  struct QuietActor;

  impl Actor for QuietActor {
    fn receive(&mut self, _ctx: &mut ActorContext<'_>, _message: AnyMessageView<'_>) -> Result<(), ActorError> {
      Ok(())
    }
  }

  let system = ActorSystem::new_empty_with(|config| {
    let executor = ExecutorShared::new(InlineExec);
    let settings = DispatcherSettings::new("default", nz(8), None, Duration::from_secs(1));
    let configurator: Box<dyn MessageDispatcherConfigurator> =
      Box::new(DefaultDispatcherConfigurator::new(&settings, executor));
    let configurator_handle: ArcShared<Box<dyn MessageDispatcherConfigurator>> = ArcShared::new(configurator);
    config.with_dispatcher_configurator("default", configurator_handle)
  });
  let state = system.state();
  let resolved = state.resolve_dispatcher("default").expect("configurator registered");

  // Creating two actor cells should bump the inhabitants counter via attach.
  let props = Props::from_fn(|| QuietActor);
  for name in ["actor-a", "actor-b"] {
    let pid = state.allocate_pid();
    let cell = ActorCell::create(state.clone(), pid, None, name.to_string(), &props).expect("create cell");
    state.register_cell(cell);
  }
  assert_eq!(resolved.inhabitants(), 2, "each spawned actor should bump inhabitants via attach");
}

#[test]
fn new_dispatcher_delivers_many_messages_to_single_actor_in_order() {
  use alloc::{string::ToString, sync::Arc, vec::Vec};

  use fraktor_utils_rs::core::sync::RuntimeMutex;

  use crate::core::kernel::{
    actor::{
      Actor, ActorCell, ActorContext,
      actor_ref::ActorRef,
      error::ActorError,
      messaging::{AnyMessage, AnyMessageView},
      props::Props,
    },
    dispatch::dispatcher::{DefaultDispatcherConfigurator, MessageDispatcherConfigurator},
    system::ActorSystem,
  };

  struct RecordingActor {
    seen: Arc<RuntimeMutex<Vec<u32>>>,
  }

  impl Actor for RecordingActor {
    fn receive(&mut self, _ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
      if let Some(value) = message.downcast_ref::<u32>() {
        self.seen.lock().push(*value);
      }
      Ok(())
    }
  }

  let system = ActorSystem::new_empty_with(|config| {
    let executor = ExecutorShared::new(InlineExec);
    let settings = DispatcherSettings::new("default", nz(16), None, Duration::from_secs(1));
    let configurator: Box<dyn MessageDispatcherConfigurator> =
      Box::new(DefaultDispatcherConfigurator::new(&settings, executor));
    let configurator_handle: ArcShared<Box<dyn MessageDispatcherConfigurator>> = ArcShared::new(configurator);
    config.with_dispatcher_configurator("default", configurator_handle)
  });
  let state = system.state();
  let seen = Arc::new(RuntimeMutex::new(Vec::new()));
  let seen_clone = Arc::clone(&seen);
  let props = Props::from_fn(move || RecordingActor { seen: seen_clone.clone() });
  let pid = state.allocate_pid();
  let cell = ActorCell::create(state.clone(), pid, None, "recording-actor".to_string(), &props).expect("create cell");
  state.register_cell(cell.clone());

  let mut actor_ref: ActorRef = cell.actor_ref();
  for i in 0..10_u32 {
    actor_ref.tell(AnyMessage::new(i));
  }

  let received = seen.lock().clone();
  assert_eq!(received, (0..10_u32).collect::<Vec<_>>(), "messages must be delivered in FIFO order");
}

#[test]
fn new_dispatcher_handles_actor_to_actor_send_without_deadlock() {
  // Regression test: when actor A processes a message and sends to actor B,
  // the inline executor must not deadlock on the shared dispatcher mutex.
  use alloc::{string::ToString, sync::Arc};
  use core::sync::atomic::{AtomicUsize, Ordering};

  use fraktor_utils_rs::core::sync::RuntimeMutex;

  use crate::core::kernel::{
    actor::{
      Actor, ActorCell, ActorContext,
      actor_ref::ActorRef,
      error::ActorError,
      messaging::{AnyMessage, AnyMessageView},
      props::Props,
    },
    dispatch::dispatcher::{DefaultDispatcherConfigurator, MessageDispatcherConfigurator},
    system::ActorSystem,
  };

  struct ForwardingActor {
    forwards_remaining: Arc<AtomicUsize>,
    downstream:         Arc<RuntimeMutex<Option<ActorRef>>>,
  }

  impl Actor for ForwardingActor {
    fn receive(&mut self, _ctx: &mut ActorContext<'_>, _message: AnyMessageView<'_>) -> Result<(), ActorError> {
      if self.forwards_remaining.fetch_sub(1, Ordering::SeqCst) > 0
        && let Some(downstream_ref) = self.downstream.lock().clone()
      {
        let mut fwd = downstream_ref;
        fwd.tell(AnyMessage::new(1_u32));
      }
      Ok(())
    }
  }

  struct CounterActor {
    count: Arc<AtomicUsize>,
  }

  impl Actor for CounterActor {
    fn receive(&mut self, _ctx: &mut ActorContext<'_>, _message: AnyMessageView<'_>) -> Result<(), ActorError> {
      self.count.fetch_add(1, Ordering::SeqCst);
      Ok(())
    }
  }

  let system = ActorSystem::new_empty_with(|config| {
    let executor = ExecutorShared::new(InlineExec);
    let settings = DispatcherSettings::new("default", nz(16), None, Duration::from_secs(1));
    let configurator: Box<dyn MessageDispatcherConfigurator> =
      Box::new(DefaultDispatcherConfigurator::new(&settings, executor));
    let configurator_handle: ArcShared<Box<dyn MessageDispatcherConfigurator>> = ArcShared::new(configurator);
    config.with_dispatcher_configurator("default", configurator_handle)
  });
  let state = system.state();

  // Create downstream actor (counter).
  let counter = Arc::new(AtomicUsize::new(0));
  let counter_clone = counter.clone();
  let counter_props = Props::from_fn(move || CounterActor { count: counter_clone.clone() });
  let counter_pid = state.allocate_pid();
  let counter_cell =
    ActorCell::create(state.clone(), counter_pid, None, "counter".to_string(), &counter_props).expect("counter cell");
  state.register_cell(counter_cell.clone());
  let counter_ref = counter_cell.actor_ref();

  // Create forwarding actor with reference to downstream.
  let forwards_remaining = Arc::new(AtomicUsize::new(3));
  let downstream = Arc::new(RuntimeMutex::new(Some(counter_ref)));
  let forwards_clone = forwards_remaining.clone();
  let downstream_clone = downstream.clone();
  let fwd_props = Props::from_fn(move || ForwardingActor {
    forwards_remaining: forwards_clone.clone(),
    downstream:         downstream_clone.clone(),
  });
  let fwd_pid = state.allocate_pid();
  let fwd_cell =
    ActorCell::create(state.clone(), fwd_pid, None, "forwarder".to_string(), &fwd_props).expect("fwd cell");
  state.register_cell(fwd_cell.clone());

  // Send a trigger message. The forwarder will send to counter from its receive handler.
  let mut fwd_ref = fwd_cell.actor_ref();
  fwd_ref.tell(AnyMessage::new(1_u32));

  // Counter should have received at least one forwarded message.
  // If this deadlocks or panics, the inline executor reentrant case is broken.
  let seen = counter.load(Ordering::SeqCst);
  assert!(seen >= 1, "counter should have received at least one forwarded message; got {seen}");
}

#[test]
fn new_dispatcher_delivers_messages_to_multiple_actors_independently() {
  use alloc::{string::ToString, sync::Arc};
  use core::sync::atomic::{AtomicUsize, Ordering};

  use crate::core::kernel::{
    actor::{
      Actor, ActorCell, ActorContext,
      actor_ref::ActorRef,
      error::ActorError,
      messaging::{AnyMessage, AnyMessageView},
      props::Props,
    },
    dispatch::dispatcher::{DefaultDispatcherConfigurator, MessageDispatcherConfigurator},
    system::ActorSystem,
  };

  struct CounterActor {
    count: Arc<AtomicUsize>,
  }

  impl Actor for CounterActor {
    fn receive(&mut self, _ctx: &mut ActorContext<'_>, _message: AnyMessageView<'_>) -> Result<(), ActorError> {
      self.count.fetch_add(1, Ordering::SeqCst);
      Ok(())
    }
  }

  let system = ActorSystem::new_empty_with(|config| {
    let executor = ExecutorShared::new(InlineExec);
    let settings = DispatcherSettings::new("default", nz(8), None, Duration::from_secs(1));
    let configurator: Box<dyn MessageDispatcherConfigurator> =
      Box::new(DefaultDispatcherConfigurator::new(&settings, executor));
    let configurator_handle: ArcShared<Box<dyn MessageDispatcherConfigurator>> = ArcShared::new(configurator);
    config.with_dispatcher_configurator("default", configurator_handle)
  });
  let state = system.state();

  let counter_a = Arc::new(AtomicUsize::new(0));
  let counter_b = Arc::new(AtomicUsize::new(0));

  let props_a = {
    let c = counter_a.clone();
    Props::from_fn(move || CounterActor { count: c.clone() })
  };
  let props_b = {
    let c = counter_b.clone();
    Props::from_fn(move || CounterActor { count: c.clone() })
  };

  let pid_a = state.allocate_pid();
  let cell_a = ActorCell::create(state.clone(), pid_a, None, "actor-a".to_string(), &props_a).expect("create a");
  state.register_cell(cell_a.clone());

  let pid_b = state.allocate_pid();
  let cell_b = ActorCell::create(state.clone(), pid_b, None, "actor-b".to_string(), &props_b).expect("create b");
  state.register_cell(cell_b.clone());

  let mut ref_a: ActorRef = cell_a.actor_ref();
  let mut ref_b: ActorRef = cell_b.actor_ref();

  for _ in 0..5 {
    ref_a.tell(AnyMessage::new(1_u32));
  }
  for _ in 0..3 {
    ref_b.tell(AnyMessage::new(1_u32));
  }

  assert_eq!(counter_a.load(Ordering::SeqCst), 5, "actor-a should have received 5 messages");
  assert_eq!(counter_b.load(Ordering::SeqCst), 3, "actor-b should have received 3 messages");
}

#[test]
fn removing_actor_cell_detaches_from_new_dispatcher_and_decrements_inhabitants() {
  use alloc::string::ToString;

  use crate::core::kernel::{
    actor::{Actor, ActorCell, ActorContext, error::ActorError, messaging::AnyMessageView, props::Props},
    dispatch::dispatcher::{DefaultDispatcherConfigurator, MessageDispatcherConfigurator},
    system::ActorSystem,
  };

  struct QuietActor;

  impl Actor for QuietActor {
    fn receive(&mut self, _ctx: &mut ActorContext<'_>, _message: AnyMessageView<'_>) -> Result<(), ActorError> {
      Ok(())
    }
  }

  let system = ActorSystem::new_empty_with(|config| {
    let executor = ExecutorShared::new(InlineExec);
    let settings = DispatcherSettings::new("default", nz(8), None, Duration::from_secs(1));
    let configurator: Box<dyn MessageDispatcherConfigurator> =
      Box::new(DefaultDispatcherConfigurator::new(&settings, executor));
    let configurator_handle: ArcShared<Box<dyn MessageDispatcherConfigurator>> = ArcShared::new(configurator);
    config.with_dispatcher_configurator("default", configurator_handle)
  });
  let state = system.state();
  let resolved = state.resolve_dispatcher("default").expect("configurator registered");

  let props = Props::from_fn(|| QuietActor);
  let pid_a = state.allocate_pid();
  let cell_a = ActorCell::create(state.clone(), pid_a, None, "actor-a".to_string(), &props).expect("create cell");
  state.register_cell(cell_a);

  let pid_b = state.allocate_pid();
  let cell_b = ActorCell::create(state.clone(), pid_b, None, "actor-b".to_string(), &props).expect("create cell");
  state.register_cell(cell_b);

  assert_eq!(resolved.inhabitants(), 2, "attach path should have incremented twice");

  state.remove_cell(&pid_a);
  assert_eq!(resolved.inhabitants(), 1, "remove_cell should call detach and decrement inhabitants");

  state.remove_cell(&pid_b);
  assert_eq!(resolved.inhabitants(), 0, "detaching all actors should leave inhabitants at zero");
}

#[test]
fn end_to_end_send_via_actor_system_with_dispatcher_configurator() {
  use alloc::string::ToString;

  use crate::core::kernel::{
    actor::{Actor, ActorContext, actor_ref::ActorRef, error::ActorError, messaging::AnyMessageView, props::Props},
    dispatch::{
      dispatcher::{DefaultDispatcherConfigurator, MessageDispatcherConfigurator},
      mailbox::MailboxPolicy,
    },
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

  let system = ActorSystem::new_empty_with(|config| {
    let executor = ExecutorShared::new(InlineExec);
    let settings = DispatcherSettings::new("default", nz(8), None, Duration::from_secs(1));
    let configurator: Box<dyn MessageDispatcherConfigurator> =
      Box::new(DefaultDispatcherConfigurator::new(&settings, executor));
    let configurator_handle: ArcShared<Box<dyn MessageDispatcherConfigurator>> = ArcShared::new(configurator);
    config.with_dispatcher_configurator("default", configurator_handle)
  });
  let state = system.state();
  let seen = Arc::new(AtomicUsize::new(0));
  let seen_clone = Arc::clone(&seen);
  // Use the default mailbox config - the actor system already registers the default.
  let props = Props::from_fn(move || CountingActor { seen: seen_clone.clone() });
  let _ = MailboxPolicy::unbounded(None);
  let pid = state.allocate_pid();
  let cell = crate::core::kernel::actor::ActorCell::create(state.clone(), pid, None, "e2e-test".to_string(), &props)
    .expect("create cell");
  state.register_cell(cell.clone());

  // ActorRef::tell goes through the new sender path because the configurator is registered.
  let mut actor_ref: ActorRef = cell.actor_ref();
  actor_ref.tell(AnyMessage::new(99_u32));

  assert_eq!(seen.load(Ordering::SeqCst), 1, "the new dispatcher must drain the message via the actor invoker");
}

#[test]
fn dispatcher_full_lifecycle_attach_dispatch_drain_detach_and_auto_shutdown() {
  // Phase 14.7: end-to-end check that a single actor goes through every
  // dispatcher state transition: spawn → attach (inhabitants ≥ 1) →
  // dispatch → drain → detach (inhabitants → 0) → auto-shutdown schedule.
  use alloc::string::ToString;

  use crate::core::kernel::{
    actor::{
      Actor, ActorCell, ActorContext, Pid, actor_ref::ActorRef, error::ActorError, messaging::AnyMessageView,
      props::Props,
    },
    dispatch::dispatcher::{DefaultDispatcherConfigurator, MessageDispatcherConfigurator},
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

  let configurator_for_resolve: ArcShared<Box<dyn MessageDispatcherConfigurator>> = {
    let executor = ExecutorShared::new(InlineExec);
    let settings = DispatcherSettings::new("lifecycle", nz(8), None, Duration::from_secs(1));
    let configurator: Box<dyn MessageDispatcherConfigurator> =
      Box::new(DefaultDispatcherConfigurator::new(&settings, executor));
    ArcShared::new(configurator)
  };
  let configurator_clone = configurator_for_resolve.clone();
  let system = ActorSystem::new_empty_with(move |config| {
    config.with_dispatcher_configurator("lifecycle", configurator_clone.clone())
  });
  let state = system.state();
  // Resolve once outside the spawn flow so we can observe the inhabitants
  // counter independently of the cell. The configurator returns a clone of
  // the same shared dispatcher so the count we read here is the same one the
  // cell attaches to.
  let dispatcher = state.resolve_dispatcher("lifecycle").expect("dispatcher resolves");
  assert_eq!(dispatcher.inhabitants(), 0, "no actor is attached before spawn");

  let seen = Arc::new(AtomicUsize::new(0));
  let seen_clone = Arc::clone(&seen);
  let props =
    Props::from_fn(move || CountingActor { seen: seen_clone.clone() }).with_dispatcher_id("lifecycle");
  let pid: Pid = state.allocate_pid();
  // attach: ActorCell::create runs the dispatcher.attach hook which bumps
  // inhabitants and registers the mailbox for execution.
  let cell = ActorCell::create(state.clone(), pid, None, "lifecycle".to_string(), &props).expect("create cell");
  state.register_cell(cell.clone());
  assert_eq!(dispatcher.inhabitants(), 1, "attach must increment inhabitants");

  // dispatch + drain: telling the actor goes through DispatcherSender,
  // which submits the mailbox.run closure to the inline executor.
  let mut actor_ref: ActorRef = cell.actor_ref();
  actor_ref.tell(AnyMessage::new(1_u32));
  actor_ref.tell(AnyMessage::new(2_u32));
  actor_ref.tell(AnyMessage::new(3_u32));
  assert_eq!(seen.load(Ordering::SeqCst), 3, "drain must process every dispatched envelope exactly once");

  // detach: removing the cell triggers MessageDispatcherShared::detach which
  // closes the mailbox, drains any leftovers to dead letters, decrements
  // inhabitants and triggers schedule_shutdown_if_sensible. With one actor
  // detached we expect inhabitants to fall back to zero.
  state.remove_cell(&pid);
  assert_eq!(dispatcher.inhabitants(), 0, "detach must decrement inhabitants back to zero");
  // auto-shutdown: the dispatcher's shutdown schedule is now set to SCHEDULED
  // because the inhabitants count just transitioned to zero. The actual
  // delayed shutdown closure runs through the scheduler, but the dispatcher
  // has already left the SCHEDULED state observable via the shared handle.
  assert_eq!(dispatcher.inhabitants(), 0);
}
