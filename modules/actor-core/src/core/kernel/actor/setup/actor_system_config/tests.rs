use alloc::boxed::Box;
use core::{
  sync::atomic::{AtomicUsize, Ordering},
  time::Duration,
};

use fraktor_utils_core_rs::core::sync::{ArcShared, SharedLock};

use crate::core::kernel::{
  actor::{
    Actor, ActorCell, ActorCellState, ActorCellStateShared, ActorCellStateSharedFactory, ActorContext,
    ActorLockFactory, ActorShared, ActorSharedFactory, ReceiveTimeoutState, ReceiveTimeoutStateShared,
    ReceiveTimeoutStateSharedFactory,
    actor_path::GuardianKind as PathGuardianKind,
    actor_ref::{ActorRefSender, ActorRefSenderShared, ActorRefSenderSharedFactory},
    actor_ref_provider::{ActorRefProviderHandleShared, ActorRefProviderHandleSharedFactory, LocalActorRefProvider},
    context_pipe::{ContextPipeWakerHandle, ContextPipeWakerHandleShared, ContextPipeWakerHandleSharedFactory},
    error::ActorError,
    messaging::{
      AnyMessageView, AskResult,
      message_invoker::{MessageInvoker, MessageInvokerShared, MessageInvokerSharedFactory},
    },
    props::Props,
    scheduler::tick_driver::{TickDriverControl, TickDriverControlShared, TickDriverControlSharedFactory},
    setup::ActorSystemConfig,
  },
  dispatch::{
    dispatcher::{
      DEFAULT_DISPATCHER_ID, Executor, ExecutorShared, ExecutorSharedFactory, MessageDispatcher,
      MessageDispatcherShared, MessageDispatcherSharedFactory, SharedMessageQueue, SharedMessageQueueFactory,
      TrampolineState,
    },
    mailbox::{
      BoundedPriorityMessageQueueState, BoundedPriorityMessageQueueStateShared,
      BoundedPriorityMessageQueueStateSharedFactory,
    },
  },
  event::stream::{
    EventStream, EventStreamShared, EventStreamSharedFactory, EventStreamSubscriber, EventStreamSubscriberShared,
    EventStreamSubscriberSharedFactory,
  },
  pattern::{CircuitBreaker, CircuitBreakerShared, CircuitBreakerSharedFactory, CircuitBreakerState, Clock},
  system::{
    ActorSystem,
    remote::RemotingConfig,
    shared_factory::{BuiltinSpinSharedFactory, MailboxSharedSet, MailboxSharedSetFactory},
  },
  util::futures::{ActorFuture, ActorFutureShared, ActorFutureSharedFactory},
};

struct NoopActor;

impl Actor for NoopActor {
  fn receive(&mut self, _ctx: &mut ActorContext<'_>, _message: AnyMessageView<'_>) -> Result<(), ActorError> {
    Ok(())
  }
}

struct CountingLockProvider {
  inner: BuiltinSpinSharedFactory,
  event_stream_shared_calls: ArcShared<AtomicUsize>,
  dispatcher_shared_calls: ArcShared<AtomicUsize>,
  executor_shared_calls: ArcShared<AtomicUsize>,
  actor_ref_sender_shared_calls: ArcShared<AtomicUsize>,
  actor_shared_lock_calls: ArcShared<AtomicUsize>,
  actor_cell_state_shared_calls: ArcShared<AtomicUsize>,
  receive_timeout_state_shared_calls: ArcShared<AtomicUsize>,
  message_invoker_shared_calls: ArcShared<AtomicUsize>,
  mailbox_shared_set_calls: ArcShared<AtomicUsize>,
  circuit_breaker_shared_calls: ArcShared<AtomicUsize>,
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
    let circuit_breaker_shared_calls = ArcShared::new(AtomicUsize::new(0));
    let provider = Self {
      inner: BuiltinSpinSharedFactory::new(),
      event_stream_shared_calls: event_stream_shared_calls.clone(),
      dispatcher_shared_calls: dispatcher_shared_calls.clone(),
      executor_shared_calls: executor_shared_calls.clone(),
      actor_ref_sender_shared_calls: actor_ref_sender_shared_calls.clone(),
      actor_shared_lock_calls: actor_shared_lock_calls.clone(),
      actor_cell_state_shared_calls: actor_cell_state_shared_calls.clone(),
      receive_timeout_state_shared_calls: receive_timeout_state_shared_calls.clone(),
      message_invoker_shared_calls: message_invoker_shared_calls.clone(),
      mailbox_shared_set_calls: mailbox_shared_set_calls.clone(),
      circuit_breaker_shared_calls: circuit_breaker_shared_calls.clone(),
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
      circuit_breaker_shared_calls,
      provider,
    )
  }
}

impl Clone for CountingLockProvider {
  fn clone(&self) -> Self {
    Self {
      inner: BuiltinSpinSharedFactory::new(),
      event_stream_shared_calls: self.event_stream_shared_calls.clone(),
      dispatcher_shared_calls: self.dispatcher_shared_calls.clone(),
      executor_shared_calls: self.executor_shared_calls.clone(),
      actor_ref_sender_shared_calls: self.actor_ref_sender_shared_calls.clone(),
      actor_shared_lock_calls: self.actor_shared_lock_calls.clone(),
      actor_cell_state_shared_calls: self.actor_cell_state_shared_calls.clone(),
      receive_timeout_state_shared_calls: self.receive_timeout_state_shared_calls.clone(),
      message_invoker_shared_calls: self.message_invoker_shared_calls.clone(),
      mailbox_shared_set_calls: self.mailbox_shared_set_calls.clone(),
      circuit_breaker_shared_calls: self.circuit_breaker_shared_calls.clone(),
    }
  }
}

#[derive(Clone)]
struct FakeClock;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct FakeInstant(u64);

impl Clock for FakeClock {
  type Instant = FakeInstant;

  fn now(&self) -> Self::Instant {
    FakeInstant(0)
  }

  fn elapsed_since(&self, _earlier: Self::Instant) -> Duration {
    Duration::ZERO
  }
}

impl ActorLockFactory for CountingLockProvider {
  fn create_lock<T>(&self, value: T) -> SharedLock<T>
  where
    T: Send + 'static, {
    self.inner.create_lock(value)
  }
}

impl MessageDispatcherSharedFactory for CountingLockProvider {
  fn create_message_dispatcher_shared(&self, dispatcher: Box<dyn MessageDispatcher>) -> MessageDispatcherShared {
    self.dispatcher_shared_calls.fetch_add(1, Ordering::SeqCst);
    MessageDispatcherSharedFactory::create_message_dispatcher_shared(&self.inner, dispatcher)
  }
}

impl ExecutorSharedFactory for CountingLockProvider {
  fn create_executor_shared(&self, executor: Box<dyn Executor>, trampoline: TrampolineState) -> ExecutorShared {
    self.executor_shared_calls.fetch_add(1, Ordering::SeqCst);
    self.inner.create_executor_shared(executor, trampoline)
  }
}

impl ActorRefSenderSharedFactory for CountingLockProvider {
  fn create_actor_ref_sender_shared(&self, sender: Box<dyn ActorRefSender>) -> ActorRefSenderShared {
    self.actor_ref_sender_shared_calls.fetch_add(1, Ordering::SeqCst);
    ActorRefSenderSharedFactory::create_actor_ref_sender_shared(&self.inner, sender)
  }
}

impl ActorSharedFactory for CountingLockProvider {
  fn create(&self, actor: Box<dyn Actor + Send>) -> ActorShared {
    self.actor_shared_lock_calls.fetch_add(1, Ordering::SeqCst);
    ActorSharedFactory::create(&self.inner, actor)
  }
}

impl BoundedPriorityMessageQueueStateSharedFactory for CountingLockProvider {
  fn create_bounded_priority_message_queue_state_shared(
    &self,
    state: BoundedPriorityMessageQueueState,
  ) -> BoundedPriorityMessageQueueStateShared {
    BoundedPriorityMessageQueueStateSharedFactory::create_bounded_priority_message_queue_state_shared(
      &self.inner,
      state,
    )
  }
}

impl ActorCellStateSharedFactory for CountingLockProvider {
  fn create_actor_cell_state_shared(&self, state: ActorCellState) -> ActorCellStateShared {
    self.actor_cell_state_shared_calls.fetch_add(1, Ordering::SeqCst);
    ActorCellStateSharedFactory::create_actor_cell_state_shared(&self.inner, state)
  }
}

impl ReceiveTimeoutStateSharedFactory for CountingLockProvider {
  fn create_receive_timeout_state_shared(&self, state: Option<ReceiveTimeoutState>) -> ReceiveTimeoutStateShared {
    self.receive_timeout_state_shared_calls.fetch_add(1, Ordering::SeqCst);
    ReceiveTimeoutStateSharedFactory::create_receive_timeout_state_shared(&self.inner, state)
  }
}

impl MessageInvokerSharedFactory for CountingLockProvider {
  fn create(&self, invoker: Box<dyn MessageInvoker>) -> MessageInvokerShared {
    self.message_invoker_shared_calls.fetch_add(1, Ordering::SeqCst);
    MessageInvokerSharedFactory::create(&self.inner, invoker)
  }
}

impl SharedMessageQueueFactory for CountingLockProvider {
  fn create(&self) -> SharedMessageQueue {
    SharedMessageQueueFactory::create(&self.inner)
  }
}

impl EventStreamSharedFactory for CountingLockProvider {
  fn create(&self, stream: EventStream) -> EventStreamShared {
    self.event_stream_shared_calls.fetch_add(1, Ordering::SeqCst);
    EventStreamSharedFactory::create(&self.inner, stream)
  }
}

impl EventStreamSubscriberSharedFactory for CountingLockProvider {
  fn create(&self, subscriber: Box<dyn EventStreamSubscriber>) -> EventStreamSubscriberShared {
    EventStreamSubscriberSharedFactory::create(&self.inner, subscriber)
  }
}

impl MailboxSharedSetFactory for CountingLockProvider {
  fn create(&self) -> MailboxSharedSet {
    self.mailbox_shared_set_calls.fetch_add(1, Ordering::SeqCst);
    MailboxSharedSetFactory::create(&self.inner)
  }
}

impl ActorFutureSharedFactory<AskResult> for CountingLockProvider {
  fn create_actor_future_shared(&self, future: ActorFuture<AskResult>) -> ActorFutureShared<AskResult> {
    ActorFutureSharedFactory::create_actor_future_shared(&self.inner, future)
  }
}

impl TickDriverControlSharedFactory for CountingLockProvider {
  fn create_tick_driver_control_shared(&self, control: Box<dyn TickDriverControl>) -> TickDriverControlShared {
    TickDriverControlSharedFactory::create_tick_driver_control_shared(&self.inner, control)
  }
}

impl ActorRefProviderHandleSharedFactory<LocalActorRefProvider> for CountingLockProvider {
  fn create_actor_ref_provider_handle_shared(
    &self,
    provider: LocalActorRefProvider,
  ) -> ActorRefProviderHandleShared<LocalActorRefProvider> {
    ActorRefProviderHandleSharedFactory::create_actor_ref_provider_handle_shared(&self.inner, provider)
  }
}

impl ContextPipeWakerHandleSharedFactory for CountingLockProvider {
  fn create_context_pipe_waker_handle_shared(&self, handle: ContextPipeWakerHandle) -> ContextPipeWakerHandleShared {
    self.inner.create_context_pipe_waker_handle_shared(handle)
  }
}

impl CircuitBreakerSharedFactory<FakeClock> for CountingLockProvider {
  fn create_circuit_breaker_shared(
    &self,
    circuit_breaker: CircuitBreaker<FakeClock>,
  ) -> CircuitBreakerShared<FakeClock> {
    self.circuit_breaker_shared_calls.fetch_add(1, Ordering::SeqCst);
    CircuitBreakerSharedFactory::create_circuit_breaker_shared(&self.inner, circuit_breaker)
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
fn test_actor_system_config_with_shared_factory_rebuilds_default_dispatcher() {
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
    _circuit_breaker_shared_calls,
    provider,
  ) = CountingLockProvider::new();

  let config = ActorSystemConfig::default().with_shared_factory(provider);

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
fn test_actor_system_config_with_shared_factory_routes_spawn_path_through_sender_and_mailbox_helpers() {
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
    _circuit_breaker_shared_calls,
    provider,
  ) = CountingLockProvider::new();
  let system = ActorSystem::new_empty_with(|config| config.with_shared_factory(provider));

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

#[test]
fn test_actor_system_config_circuit_breaker_shared_factory_uses_registered_provider() {
  let (
    _event_stream_shared_calls,
    _dispatcher_shared_calls,
    _executor_shared_calls,
    _actor_ref_sender_shared_calls,
    _actor_shared_lock_calls,
    _actor_cell_state_shared_calls,
    _receive_timeout_state_shared_calls,
    _message_invoker_shared_calls,
    _mailbox_shared_set_calls,
    circuit_breaker_shared_calls,
    provider,
  ) = CountingLockProvider::new();

  let config = ActorSystemConfig::default()
    .with_shared_factory(provider.clone())
    .with_circuit_breaker_shared_factory::<FakeClock, _>(provider);

  let shared = config
    .circuit_breaker_shared_factory::<FakeClock>()
    .expect("circuit breaker shared factory should be registered for FakeClock")
    .create_circuit_breaker_shared(CircuitBreaker::new_with_clock(2, Duration::from_secs(1), FakeClock));

  assert_eq!(shared.state(), CircuitBreakerState::Closed);
  assert_eq!(
    circuit_breaker_shared_calls.load(Ordering::SeqCst),
    1,
    "ActorSystemConfig should materialize CircuitBreakerShared via the registered provider"
  );
}
