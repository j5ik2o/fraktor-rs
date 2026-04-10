use alloc::boxed::Box;
use core::{
  sync::atomic::{AtomicUsize, Ordering},
  time::Duration,
};

use fraktor_utils_core_rs::core::sync::{ArcShared, SharedLock};

use crate::core::kernel::{
  actor::{
    Actor, ActorCell, ActorCellStateShared, ActorContext, ReceiveTimeoutStateShared,
    actor_path::GuardianKind as PathGuardianKind,
    actor_ref::{ActorRefSender, ActorRefSenderShared},
    error::ActorError,
    messaging::{
      AnyMessageView,
      message_invoker::{MessageInvoker, MessageInvokerShared},
    },
    props::Props,
    setup::ActorSystemConfig,
  },
  dispatch::dispatcher::{
    DEFAULT_DISPATCHER_ID, Executor, ExecutorShared, MessageDispatcher, MessageDispatcherShared, SharedMessageQueue,
  },
  event::stream::{EventStream, EventStreamShared, EventStreamSubscriber, EventStreamSubscriberShared},
  system::{
    ActorSystem,
    lock_provider::{ActorLockProvider, BuiltinSpinLockProvider, MailboxSharedSet},
    remote::RemotingConfig,
  },
};

struct NoopActor;

impl Actor for NoopActor {
  fn receive(&mut self, _ctx: &mut ActorContext<'_>, _message: AnyMessageView<'_>) -> Result<(), ActorError> {
    Ok(())
  }
}

struct CountingLockProvider {
  inner: BuiltinSpinLockProvider,
  event_stream_shared_calls: ArcShared<AtomicUsize>,
  dispatcher_shared_calls: ArcShared<AtomicUsize>,
  executor_shared_calls: ArcShared<AtomicUsize>,
  actor_ref_sender_shared_calls: ArcShared<AtomicUsize>,
  actor_shared_lock_calls: ArcShared<AtomicUsize>,
  actor_cell_state_shared_calls: ArcShared<AtomicUsize>,
  receive_timeout_state_shared_calls: ArcShared<AtomicUsize>,
  message_invoker_shared_calls: ArcShared<AtomicUsize>,
  mailbox_shared_set_calls: ArcShared<AtomicUsize>,
}

impl CountingLockProvider {
  fn new() -> (
    ArcShared<AtomicUsize>,
    ArcShared<AtomicUsize>,
    ArcShared<AtomicUsize>,
    ArcShared<AtomicUsize>,
    ArcShared<AtomicUsize>,
    ArcShared<AtomicUsize>,
    ArcShared<AtomicUsize>,
    ArcShared<AtomicUsize>,
    ArcShared<AtomicUsize>,
    Self,
  ) {
    let event_stream_shared_calls = ArcShared::new(AtomicUsize::new(0));
    let dispatcher_shared_calls = ArcShared::new(AtomicUsize::new(0));
    let executor_shared_calls = ArcShared::new(AtomicUsize::new(0));
    let actor_ref_sender_shared_calls = ArcShared::new(AtomicUsize::new(0));
    let actor_shared_lock_calls = ArcShared::new(AtomicUsize::new(0));
    let actor_cell_state_shared_calls = ArcShared::new(AtomicUsize::new(0));
    let receive_timeout_state_shared_calls = ArcShared::new(AtomicUsize::new(0));
    let message_invoker_shared_calls = ArcShared::new(AtomicUsize::new(0));
    let mailbox_shared_set_calls = ArcShared::new(AtomicUsize::new(0));
    let provider = Self {
      inner: BuiltinSpinLockProvider::new(),
      event_stream_shared_calls: event_stream_shared_calls.clone(),
      dispatcher_shared_calls: dispatcher_shared_calls.clone(),
      executor_shared_calls: executor_shared_calls.clone(),
      actor_ref_sender_shared_calls: actor_ref_sender_shared_calls.clone(),
      actor_shared_lock_calls: actor_shared_lock_calls.clone(),
      actor_cell_state_shared_calls: actor_cell_state_shared_calls.clone(),
      receive_timeout_state_shared_calls: receive_timeout_state_shared_calls.clone(),
      message_invoker_shared_calls: message_invoker_shared_calls.clone(),
      mailbox_shared_set_calls: mailbox_shared_set_calls.clone(),
    };
    (
      event_stream_shared_calls,
      dispatcher_shared_calls,
      executor_shared_calls,
      actor_ref_sender_shared_calls,
      actor_shared_lock_calls,
      actor_cell_state_shared_calls,
      receive_timeout_state_shared_calls,
      message_invoker_shared_calls,
      mailbox_shared_set_calls,
      provider,
    )
  }
}

impl ActorLockProvider for CountingLockProvider {
  fn create_message_dispatcher_shared(&self, dispatcher: Box<dyn MessageDispatcher>) -> MessageDispatcherShared {
    self.dispatcher_shared_calls.fetch_add(1, Ordering::SeqCst);
    self.inner.create_message_dispatcher_shared(dispatcher)
  }

  fn create_executor_shared(&self, executor: Box<dyn Executor>) -> ExecutorShared {
    self.executor_shared_calls.fetch_add(1, Ordering::SeqCst);
    self.inner.create_executor_shared(executor)
  }

  fn create_actor_ref_sender_shared(&self, sender: Box<dyn ActorRefSender>) -> ActorRefSenderShared {
    self.actor_ref_sender_shared_calls.fetch_add(1, Ordering::SeqCst);
    self.inner.create_actor_ref_sender_shared(sender)
  }

  fn create_actor_shared_lock(&self, actor: Box<dyn Actor + Send + Sync>) -> SharedLock<Box<dyn Actor + Send + Sync>> {
    self.actor_shared_lock_calls.fetch_add(1, Ordering::SeqCst);
    self.inner.create_actor_shared_lock(actor)
  }

  fn create_actor_cell_state_shared(&self) -> ActorCellStateShared {
    self.actor_cell_state_shared_calls.fetch_add(1, Ordering::SeqCst);
    self.inner.create_actor_cell_state_shared()
  }

  fn create_receive_timeout_state_shared(&self) -> ReceiveTimeoutStateShared {
    self.receive_timeout_state_shared_calls.fetch_add(1, Ordering::SeqCst);
    self.inner.create_receive_timeout_state_shared()
  }

  fn create_message_invoker_shared(&self, invoker: Box<dyn MessageInvoker>) -> MessageInvokerShared {
    self.message_invoker_shared_calls.fetch_add(1, Ordering::SeqCst);
    self.inner.create_message_invoker_shared(invoker)
  }

  fn create_shared_message_queue(&self) -> SharedMessageQueue {
    self.inner.create_shared_message_queue()
  }

  fn create_event_stream_shared(&self, stream: EventStream) -> EventStreamShared {
    self.event_stream_shared_calls.fetch_add(1, Ordering::SeqCst);
    self.inner.create_event_stream_shared(stream)
  }

  fn create_event_stream_subscriber_shared(
    &self,
    subscriber: Box<dyn EventStreamSubscriber>,
  ) -> EventStreamSubscriberShared {
    self.inner.create_event_stream_subscriber_shared(subscriber)
  }

  fn create_mailbox_shared_set(&self) -> MailboxSharedSet {
    self.mailbox_shared_set_calls.fetch_add(1, Ordering::SeqCst);
    self.inner.create_mailbox_shared_set()
  }
}

#[test]
fn test_actor_system_config_default() {
  let config = ActorSystemConfig::default();
  assert_eq!(config.system_name(), "default-system");
  assert_eq!(config.default_guardian(), PathGuardianKind::User);
  assert!(config.remoting_config().is_none());
}

#[test]
fn test_actor_system_config_with_system_name() {
  let config = ActorSystemConfig::default().with_system_name("test-system");
  assert_eq!(config.system_name(), "test-system");
}

#[test]
fn test_actor_system_config_with_default_guardian() {
  let config = ActorSystemConfig::default().with_default_guardian(PathGuardianKind::System);
  assert_eq!(config.default_guardian(), PathGuardianKind::System);
}

#[test]
fn test_actor_system_config_with_remoting() {
  let remoting = RemotingConfig::default().with_canonical_host("localhost").with_canonical_port(2552);

  let config = ActorSystemConfig::default().with_remoting_config(remoting);

  assert!(config.remoting_config().is_some());
  let remoting_cfg = config.remoting_config().unwrap();
  assert_eq!(remoting_cfg.canonical_host(), "localhost");
  assert_eq!(remoting_cfg.canonical_port(), Some(2552));
}

#[test]
fn test_remoting_config_quarantine_duration() {
  let custom_duration = Duration::from_secs(1800);
  let remoting = RemotingConfig::default().with_quarantine_duration(custom_duration);

  assert_eq!(remoting.quarantine_duration(), custom_duration);
}

#[test]
fn test_remoting_config_defaults() {
  let remoting = RemotingConfig::default();

  // デフォルト値の検証
  assert_eq!(remoting.canonical_host(), "localhost");
  assert_eq!(remoting.canonical_port(), None);
  assert_eq!(remoting.quarantine_duration(), Duration::from_secs(5 * 24 * 3600)); // 5日
}

#[test]
#[should_panic(expected = "quarantine duration must be >= 1 second")]
fn test_remoting_config_rejects_short_quarantine() {
  drop(RemotingConfig::default().with_quarantine_duration(Duration::from_millis(999)));
}

#[test]
fn test_actor_system_config_default_resolves_default_dispatcher() {
  let config = ActorSystemConfig::default();
  assert!(
    config.dispatchers().resolve(DEFAULT_DISPATCHER_ID).is_ok(),
    "ActorSystemConfig::default() should seed the default dispatcher entry"
  );
}

#[test]
fn test_actor_system_config_with_lock_provider_rebuilds_default_dispatcher() {
  let (
    _event_stream_shared_calls,
    dispatcher_shared_calls,
    executor_shared_calls,
    _actor_ref_sender_shared_calls,
    _actor_shared_lock_calls,
    _actor_cell_state_shared_calls,
    _receive_timeout_state_shared_calls,
    _message_invoker_shared_calls,
    _mailbox_shared_set_calls,
    provider,
  ) = CountingLockProvider::new();

  let config = ActorSystemConfig::default().with_lock_provider(provider);

  assert_eq!(
    executor_shared_calls.load(Ordering::SeqCst),
    1,
    "Replacing the lock provider should rebuild the default dispatcher executor wrapper"
  );
  assert_eq!(
    dispatcher_shared_calls.load(Ordering::SeqCst),
    1,
    "Replacing the lock provider should rebuild the default dispatcher shared wrapper"
  );
  assert!(
    config.dispatchers().resolve(DEFAULT_DISPATCHER_ID).is_ok(),
    "Rebuilt default dispatcher should remain resolvable"
  );
}

#[test]
fn test_actor_system_config_with_lock_provider_routes_spawn_path_through_sender_and_mailbox_helpers() {
  let (
    event_stream_shared_calls,
    _dispatcher_shared_calls,
    _executor_shared_calls,
    actor_ref_sender_shared_calls,
    actor_shared_lock_calls,
    actor_cell_state_shared_calls,
    receive_timeout_state_shared_calls,
    message_invoker_shared_calls,
    mailbox_shared_set_calls,
    provider,
  ) = CountingLockProvider::new();
  let system = ActorSystem::new_empty_with(|config| config.with_lock_provider(provider));

  let props = Props::from_fn(|| NoopActor);
  let state = system.state();
  let pid = state.allocate_pid();
  let cell = ActorCell::create(state.clone(), pid, None, "counting-actor".into(), &props).expect("counting actor");
  state.register_cell(cell);

  assert_eq!(
    event_stream_shared_calls.load(Ordering::SeqCst),
    1,
    "system bootstrap should materialize EventStreamShared via the configured lock provider"
  );
  assert_eq!(
    actor_ref_sender_shared_calls.load(Ordering::SeqCst),
    1,
    "spawn path should materialize ActorRefSenderShared via the configured lock provider"
  );
  assert_eq!(
    actor_shared_lock_calls.load(Ordering::SeqCst),
    1,
    "spawn path should materialize the actor instance lock via the configured lock provider"
  );
  assert_eq!(
    actor_cell_state_shared_calls.load(Ordering::SeqCst),
    1,
    "spawn path should materialize ActorCellStateShared via the configured lock provider"
  );
  assert_eq!(
    receive_timeout_state_shared_calls.load(Ordering::SeqCst),
    1,
    "spawn path should materialize ReceiveTimeoutStateShared via the configured lock provider"
  );
  assert_eq!(
    message_invoker_shared_calls.load(Ordering::SeqCst),
    1,
    "spawn path should materialize the mailbox invoker via the configured lock provider"
  );
  assert_eq!(
    mailbox_shared_set_calls.load(Ordering::SeqCst),
    1,
    "spawn path should materialize mailbox shared locks via the configured lock provider"
  );
}
