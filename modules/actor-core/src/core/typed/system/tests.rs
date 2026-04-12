extern crate alloc;

use alloc::{boxed::Box, vec::Vec};
use core::{
  sync::atomic::{AtomicUsize, Ordering},
  time::Duration,
};

use fraktor_utils_core_rs::core::sync::{ArcShared, SharedLock, SpinSyncMutex};

use crate::core::{
  kernel::{
    actor::{
      Actor, ActorCellState, ActorCellStateShared, ActorCellStateSharedFactory, ActorSharedLockFactory, Pid,
      ReceiveTimeoutState, ReceiveTimeoutStateShared, ReceiveTimeoutStateSharedFactory,
      actor_ref::{
        ActorRef, ActorRefSender, ActorRefSenderShared, ActorRefSenderSharedFactory, SendOutcome,
        dead_letter::DeadLetterReason,
      },
      actor_ref_provider::{ActorRefProviderHandleShared, ActorRefProviderHandleSharedFactory, LocalActorRefProvider},
      context_pipe::{ContextPipeWakerHandle, ContextPipeWakerHandleShared, ContextPipeWakerHandleSharedFactory},
      error::SendError,
      extension::{Extension, ExtensionId},
      messaging::{
        AnyMessage, AskResult,
        message_invoker::{MessageInvoker, MessageInvokerShared, MessageInvokerSharedFactory},
      },
      scheduler::tick_driver::{
        ManualTestDriver, TickDriverConfig, TickDriverControl, TickDriverControlShared, TickDriverControlSharedFactory,
      },
      setup::ActorSystemConfig,
    },
    dispatch::dispatcher::{
      Executor, ExecutorShared, ExecutorSharedFactory, MessageDispatcher, MessageDispatcherShared,
      MessageDispatcherSharedFactory, SharedMessageQueue, SharedMessageQueueFactory, TrampolineState,
    },
    event::{
      logging::{LogEvent, LogLevel},
      stream::{
        EventStream, EventStreamEvent, EventStreamShared, EventStreamSharedFactory, EventStreamSubscriber,
        EventStreamSubscriberShared, EventStreamSubscriberSharedFactory, tests::subscriber_handle,
      },
    },
    system::{
      ActorSystem,
      shared_factory::{BuiltinSpinSharedFactory, MailboxSharedSet, MailboxSharedSetFactory},
    },
    util::futures::{ActorFuture, ActorFutureShared, ActorFutureSharedFactory},
  },
  typed::{
    DispatcherSelector, TypedActorRef, TypedActorSystem, TypedProps,
    dsl::Behaviors,
    eventstream::EventStreamCommand,
    receptionist::{ReceptionistCommand, SYSTEM_RECEPTIONIST_TOP_LEVEL},
  },
};

struct TestExtension {
  value: u32,
}

impl Extension for TestExtension {}

struct TestExtensionId {
  initial_value: u32,
}

impl ExtensionId for TestExtensionId {
  type Ext = TestExtension;

  fn create_extension(&self, _system: &ActorSystem) -> Self::Ext {
    TestExtension { value: self.initial_value }
  }
}

struct RecordingSubscriber {
  events: ArcShared<SpinSyncMutex<Vec<EventStreamEvent>>>,
}

impl RecordingSubscriber {
  fn new(events: ArcShared<SpinSyncMutex<Vec<EventStreamEvent>>>) -> Self {
    Self { events }
  }
}

impl EventStreamSubscriber for RecordingSubscriber {
  fn on_event(&mut self, event: &EventStreamEvent) {
    self.events.lock().push(event.clone());
  }
}

struct CollectorSender {
  events: ArcShared<SpinSyncMutex<Vec<EventStreamEvent>>>,
}

impl CollectorSender {
  fn new(events: ArcShared<SpinSyncMutex<Vec<EventStreamEvent>>>) -> Self {
    Self { events }
  }
}

impl ActorRefSender for CollectorSender {
  fn send(&mut self, message: AnyMessage) -> Result<SendOutcome, SendError> {
    if let Some(event) = message.payload().downcast_ref::<EventStreamEvent>() {
      self.events.lock().push(event.clone());
    }
    Ok(SendOutcome::Delivered)
  }
}

struct CountingSubscriberLockProvider {
  inner: BuiltinSpinSharedFactory,
  event_stream_subscriber_shared: ArcShared<AtomicUsize>,
}

impl CountingSubscriberLockProvider {
  fn new() -> (ArcShared<AtomicUsize>, Self) {
    let event_stream_subscriber_shared = ArcShared::new(AtomicUsize::new(0));
    let provider = Self {
      inner: BuiltinSpinSharedFactory::new(),
      event_stream_subscriber_shared: event_stream_subscriber_shared.clone(),
    };
    (event_stream_subscriber_shared, provider)
  }
}

impl MessageDispatcherSharedFactory for CountingSubscriberLockProvider {
  fn create_message_dispatcher_shared(&self, dispatcher: Box<dyn MessageDispatcher>) -> MessageDispatcherShared {
    MessageDispatcherSharedFactory::create_message_dispatcher_shared(&self.inner, dispatcher)
  }
}

impl ExecutorSharedFactory for CountingSubscriberLockProvider {
  fn create_executor_shared(&self, executor: Box<dyn Executor>, trampoline: TrampolineState) -> ExecutorShared {
    self.inner.create_executor_shared(executor, trampoline)
  }
}

impl ActorRefSenderSharedFactory for CountingSubscriberLockProvider {
  fn create_actor_ref_sender_shared(&self, sender: Box<dyn ActorRefSender>) -> ActorRefSenderShared {
    ActorRefSenderSharedFactory::create_actor_ref_sender_shared(&self.inner, sender)
  }
}

impl ActorSharedLockFactory for CountingSubscriberLockProvider {
  fn create(&self, actor: Box<dyn Actor + Send>) -> SharedLock<Box<dyn Actor + Send>> {
    ActorSharedLockFactory::create(&self.inner, actor)
  }
}

impl ActorCellStateSharedFactory for CountingSubscriberLockProvider {
  fn create_actor_cell_state_shared(&self, state: ActorCellState) -> ActorCellStateShared {
    ActorCellStateSharedFactory::create_actor_cell_state_shared(&self.inner, state)
  }
}

impl ReceiveTimeoutStateSharedFactory for CountingSubscriberLockProvider {
  fn create_receive_timeout_state_shared(&self, state: Option<ReceiveTimeoutState>) -> ReceiveTimeoutStateShared {
    ReceiveTimeoutStateSharedFactory::create_receive_timeout_state_shared(&self.inner, state)
  }
}

impl MessageInvokerSharedFactory for CountingSubscriberLockProvider {
  fn create(&self, invoker: Box<dyn MessageInvoker>) -> MessageInvokerShared {
    MessageInvokerSharedFactory::create(&self.inner, invoker)
  }
}

impl SharedMessageQueueFactory for CountingSubscriberLockProvider {
  fn create(&self) -> SharedMessageQueue {
    SharedMessageQueueFactory::create(&self.inner)
  }
}

impl EventStreamSharedFactory for CountingSubscriberLockProvider {
  fn create(&self, stream: EventStream) -> EventStreamShared {
    EventStreamSharedFactory::create(&self.inner, stream)
  }
}

impl EventStreamSubscriberSharedFactory for CountingSubscriberLockProvider {
  fn create(&self, subscriber: Box<dyn EventStreamSubscriber>) -> EventStreamSubscriberShared {
    self.event_stream_subscriber_shared.fetch_add(1, Ordering::SeqCst);
    EventStreamSubscriberSharedFactory::create(&self.inner, subscriber)
  }
}

impl MailboxSharedSetFactory for CountingSubscriberLockProvider {
  fn create(&self) -> MailboxSharedSet {
    MailboxSharedSetFactory::create(&self.inner)
  }
}

impl ActorFutureSharedFactory<AskResult> for CountingSubscriberLockProvider {
  fn create_actor_future_shared(&self, future: ActorFuture<AskResult>) -> ActorFutureShared<AskResult> {
    ActorFutureSharedFactory::create_actor_future_shared(&self.inner, future)
  }
}

impl TickDriverControlSharedFactory for CountingSubscriberLockProvider {
  fn create_tick_driver_control_shared(&self, control: Box<dyn TickDriverControl>) -> TickDriverControlShared {
    TickDriverControlSharedFactory::create_tick_driver_control_shared(&self.inner, control)
  }
}

impl ActorRefProviderHandleSharedFactory<LocalActorRefProvider> for CountingSubscriberLockProvider {
  fn create_actor_ref_provider_handle_shared(
    &self,
    provider: LocalActorRefProvider,
  ) -> ActorRefProviderHandleShared<LocalActorRefProvider> {
    ActorRefProviderHandleSharedFactory::create_actor_ref_provider_handle_shared(&self.inner, provider)
  }
}

impl ContextPipeWakerHandleSharedFactory for CountingSubscriberLockProvider {
  fn create_context_pipe_waker_handle_shared(&self, handle: ContextPipeWakerHandle) -> ContextPipeWakerHandleShared {
    self.inner.create_context_pipe_waker_handle_shared(handle)
  }
}

fn new_test_system() -> TypedActorSystem<u32> {
  let guardian_props = TypedProps::<u32>::from_behavior_factory(Behaviors::ignore);
  let tick_driver = TickDriverConfig::manual(ManualTestDriver::new());
  let config = ActorSystemConfig::default().with_start_time(Duration::from_secs(1)).with_tick_driver(tick_driver);
  TypedActorSystem::<u32>::new_with_config(&guardian_props, &config).expect("system")
}

// --- T5: Extension facade tests ---

#[test]
fn register_extension_returns_created_instance() {
  // Given: a typed actor system and an extension id
  let system = new_test_system();
  let ext_id = TestExtensionId { initial_value: 42 };

  // When: register_extension is called
  let ext = system.register_extension(&ext_id);

  // Then: the created extension is returned with the initial value
  assert_eq!(ext.value, 42);

  system.terminate().expect("terminate");
}

#[test]
fn has_extension_returns_false_before_registration() {
  // Given: a typed actor system with no extensions registered
  let system = new_test_system();
  let ext_id = TestExtensionId { initial_value: 0 };

  // When/Then: has_extension returns false
  assert!(!system.has_extension(&ext_id));

  system.terminate().expect("terminate");
}

#[test]
fn has_extension_returns_true_after_registration() {
  // Given: a typed actor system with an extension registered
  let system = new_test_system();
  let ext_id = TestExtensionId { initial_value: 0 };
  system.register_extension(&ext_id);

  // When/Then: has_extension returns true
  assert!(system.has_extension(&ext_id));

  system.terminate().expect("terminate");
}

#[test]
fn extension_returns_none_before_registration() {
  // Given: a typed actor system with no extensions registered
  let system = new_test_system();
  let ext_id = TestExtensionId { initial_value: 0 };

  // When/Then: extension returns None
  let result: Option<ArcShared<TestExtension>> = system.extension(&ext_id);
  assert!(result.is_none());

  system.terminate().expect("terminate");
}

#[test]
fn extension_returns_registered_instance() {
  // Given: a typed actor system with an extension registered
  let system = new_test_system();
  let ext_id = TestExtensionId { initial_value: 99 };
  system.register_extension(&ext_id);

  // When: extension is called
  let result: Option<ArcShared<TestExtension>> = system.extension(&ext_id);

  // Then: the registered instance is returned
  let ext = result.expect("extension should be present");
  assert_eq!(ext.value, 99);

  system.terminate().expect("terminate");
}

#[test]
fn register_extension_is_idempotent() {
  // Given: a typed actor system with an extension already registered
  let system = new_test_system();
  let ext_id = TestExtensionId { initial_value: 10 };
  let first = system.register_extension(&ext_id);

  // When: register_extension is called again with the same id
  let second = system.register_extension(&ext_id);

  // Then: the same instance is returned (putIfAbsent semantics)
  assert_eq!(first.value, second.value);

  system.terminate().expect("terminate");
}

// --- T10: TypedActorSystem metadata accessors ---

#[test]
fn name_returns_configured_system_name() {
  // Given: a typed actor system with a custom name
  let guardian_props = TypedProps::<u32>::from_behavior_factory(Behaviors::ignore);
  let config = ActorSystemConfig::default()
    .with_system_name("my-actor-system")
    .with_tick_driver(TickDriverConfig::manual(ManualTestDriver::new()));
  let system = TypedActorSystem::<u32>::new_with_config(&guardian_props, &config).expect("system");

  // When: name() is called
  let name = system.name();

  // Then: the configured name is returned
  assert_eq!(name, "my-actor-system");

  system.terminate().expect("terminate");
}

#[test]
fn name_returns_default_name_when_not_configured() {
  // Given: a typed actor system with default configuration
  let system = new_test_system();

  // When: name() is called
  let name = system.name();

  // Then: a non-empty default name is returned
  assert!(!name.is_empty(), "default system name should not be empty");

  system.terminate().expect("terminate");
}

#[test]
fn start_time_returns_non_zero_duration() {
  // Given: a typed actor system
  let system = new_test_system();

  // When: start_time() is called
  let start_time = system.start_time();

  // Then: start_time is non-zero
  assert_ne!(start_time, Duration::ZERO, "start_time should be non-zero");

  system.terminate().expect("terminate");
}

#[test]
fn start_time_returns_configured_value() {
  // Given: a typed actor system with an explicit start_time
  let expected_start = Duration::from_secs(1_700_000_000);
  let guardian_props = TypedProps::<u32>::from_behavior_factory(Behaviors::ignore);
  let config = ActorSystemConfig::default()
    .with_start_time(expected_start)
    .with_tick_driver(TickDriverConfig::manual(ManualTestDriver::new()));
  let system = TypedActorSystem::<u32>::new_with_config(&guardian_props, &config).expect("system");

  // When: start_time() is called
  let start_time = system.start_time();

  // Then: the configured start_time is returned
  assert_eq!(start_time, expected_start);

  system.terminate().expect("terminate");
}

#[test]
fn uptime_returns_elapsed_since_start() {
  // Given: a system with a known start_time
  let start = Duration::from_secs(1_000);
  let guardian_props = TypedProps::<u32>::from_behavior_factory(Behaviors::ignore);
  let config = ActorSystemConfig::default()
    .with_start_time(start)
    .with_tick_driver(TickDriverConfig::manual(ManualTestDriver::new()));
  let system = TypedActorSystem::<u32>::new_with_config(&guardian_props, &config).expect("system");

  // When: uptime is calculated with a known "now"
  let now = Duration::from_secs(1_042);
  let uptime = system.uptime(now);

  // Then: uptime = now - start_time
  assert_eq!(uptime, Duration::from_secs(42));

  system.terminate().expect("terminate");
}

#[test]
fn uptime_saturates_when_now_is_before_start() {
  // Given: a system with a start_time
  let start = Duration::from_secs(1_000);
  let guardian_props = TypedProps::<u32>::from_behavior_factory(Behaviors::ignore);
  let config = ActorSystemConfig::default()
    .with_start_time(start)
    .with_tick_driver(TickDriverConfig::manual(ManualTestDriver::new()));
  let system = TypedActorSystem::<u32>::new_with_config(&guardian_props, &config).expect("system");

  // When: now is before start_time (edge case)
  let now = Duration::from_secs(500);
  let uptime = system.uptime(now);

  // Then: uptime saturates to zero
  assert_eq!(uptime, Duration::ZERO);

  system.terminate().expect("terminate");
}

// --- T12: TypedActorSystem.scheduler() returns Scheduler facade ---

#[test]
fn scheduler_returns_facade_that_can_schedule_once() {
  // Given: a typed actor system
  let system = new_test_system();

  // When: scheduler() is called and schedule_once is invoked
  let scheduler = system.scheduler();
  let receiver = TypedActorRef::<u32>::from_untyped(ActorRef::null());
  let result = scheduler.schedule_once(Duration::from_millis(10), receiver, 42u32);

  // Then: a SchedulerHandle is returned
  assert!(result.is_ok(), "scheduler().schedule_once() should succeed");

  system.terminate().expect("terminate");
}

#[test]
fn scheduler_returns_facade_that_can_schedule_at_fixed_rate() {
  // Given: a typed actor system
  let system = new_test_system();

  // When: scheduler() returns a Scheduler and schedule_at_fixed_rate is called
  let scheduler = system.scheduler();
  let receiver = TypedActorRef::<u32>::from_untyped(ActorRef::null());
  let result = scheduler.schedule_at_fixed_rate(Duration::from_millis(5), Duration::from_millis(10), receiver, 7u32);

  // Then: a SchedulerHandle is returned
  assert!(result.is_ok(), "scheduler().schedule_at_fixed_rate() should succeed");

  system.terminate().expect("terminate");
}

#[test]
fn scheduler_returns_facade_that_can_schedule_with_fixed_delay() {
  // Given: a typed actor system
  let system = new_test_system();

  // When: scheduler() returns a Scheduler and schedule_with_fixed_delay is called
  let scheduler = system.scheduler();
  let receiver = TypedActorRef::<u32>::from_untyped(ActorRef::null());
  let result =
    scheduler.schedule_with_fixed_delay(Duration::from_millis(5), Duration::from_millis(20), receiver, 99u32);

  // Then: a SchedulerHandle is returned
  assert!(result.is_ok(), "scheduler().schedule_with_fixed_delay() should succeed");

  system.terminate().expect("terminate");
}

// --- T11: TypedActorSystem.dispatchers() accessor ---

#[test]
fn dispatchers_returns_facade_that_resolves_default() {
  // Given: a typed actor system
  let system = new_test_system();

  // When: dispatchers() is called and the default selector is looked up
  let dispatchers = system.dispatchers();
  let result = dispatchers.lookup(&DispatcherSelector::Default);

  // Then: the default dispatcher configuration is returned
  assert!(result.is_ok(), "dispatchers().lookup(Default) should succeed");

  system.terminate().expect("terminate");
}

#[test]
fn dispatchers_returns_facade_that_resolves_blocking() {
  // Given: a typed actor system
  let system = new_test_system();

  // When: dispatchers() is called and the Blocking selector is looked up
  let dispatchers = system.dispatchers();
  let result = dispatchers.lookup(&DispatcherSelector::Blocking);

  // Then: the blocking dispatcher configuration is returned
  assert!(result.is_ok(), "dispatchers().lookup(Blocking) should succeed");

  system.terminate().expect("terminate");
}

#[test]
fn dispatchers_returns_facade_that_rejects_unknown() {
  // Given: a typed actor system
  let system = new_test_system();

  // When: dispatchers() is called and an unknown dispatcher id is looked up
  let dispatchers = system.dispatchers();
  let selector = DispatcherSelector::from_config("nonexistent");
  let result = dispatchers.lookup(&selector);

  // Then: an error is returned
  assert!(result.is_err(), "dispatchers().lookup(unknown) should fail");

  system.terminate().expect("terminate");
}

// --- TypedActorSystem.address() accessor ---

#[test]
fn address_returns_local_address_with_system_name() {
  // Given: a typed actor system with default configuration
  let system = new_test_system();

  // When: address() is called
  let address = system.address();

  // Then: the address has the system name and local scope
  assert_eq!(address.system(), system.name());
  assert!(address.has_local_scope(), "default address should be local");

  system.terminate().expect("terminate");
}

#[test]
fn address_returns_local_address_with_custom_name() {
  // Given: a typed actor system with a custom name
  let guardian_props = TypedProps::<u32>::from_behavior_factory(Behaviors::ignore);
  let config = ActorSystemConfig::default()
    .with_system_name("custom-system")
    .with_tick_driver(TickDriverConfig::manual(ManualTestDriver::new()));
  let system = TypedActorSystem::<u32>::new_with_config(&guardian_props, &config).expect("system");

  // When: address() is called
  let address = system.address();

  // Then: the address reflects the custom name
  assert_eq!(address.system(), "custom-system");
  assert!(address.has_local_scope());
  assert!(!address.has_global_scope());

  system.terminate().expect("terminate");
}

#[test]
fn address_uses_fraktor_protocol() {
  // Given: a typed actor system
  let system = new_test_system();

  // When: address() is called
  let address = system.address();

  // Then: the protocol is the fraktor default
  assert_eq!(address.protocol(), "fraktor");

  system.terminate().expect("terminate");
}

#[test]
fn settings_returns_snapshot_preserved_through_from_untyped() {
  // Given: an untyped actor system with configured metadata
  let expected_start_time = Duration::from_secs(1_234);
  let untyped = ActorSystem::new_empty_with(|config| {
    config.with_system_name("wrapped-system").with_start_time(expected_start_time)
  });
  let system = TypedActorSystem::<u32>::from_untyped(untyped);

  // When: settings() is called on the typed wrapper
  let settings = system.settings();

  // Then: the immutable snapshot preserves the source configuration
  assert_eq!(settings.system_name(), "wrapped-system");
  assert_eq!(settings.start_time(), expected_start_time);
}

#[test]
fn receptionist_returns_registered_system_receptionist_ref() {
  let system = new_test_system();

  let receptionist_ref = system.receptionist_ref();
  let receptionist = system.receptionist();
  let top_level =
    system.state().extra_top_level(SYSTEM_RECEPTIONIST_TOP_LEVEL).expect("system receptionist top-level registration");
  let top_level = TypedActorRef::<ReceptionistCommand>::from_untyped(top_level);

  assert_eq!(receptionist_ref.expect("registered receptionist").pid(), top_level.pid());
  assert_eq!(receptionist.pid(), top_level.pid());

  system.terminate().expect("terminate");
}

#[test]
fn receptionist_ref_returns_none_when_missing() {
  let system = TypedActorSystem::<u32>::from_untyped(ActorSystem::new_empty());

  assert!(system.receptionist_ref().is_none());
}

#[test]
#[should_panic(expected = "system receptionist must be installed during actor system bootstrap")]
fn receptionist_panics_when_missing() {
  let system = TypedActorSystem::<u32>::from_untyped(ActorSystem::new_empty());
  let _ = system.receptionist();
}

#[test]
fn log_configuration_emits_log_event_to_event_stream() {
  // Given: a typed actor system with a recording subscriber
  let system = new_test_system();
  let events = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let subscriber = subscriber_handle(RecordingSubscriber::new(events.clone()));
  let _subscription = system.subscribe_event_stream(&subscriber);
  let baseline_len = events.lock().len();

  // When: log_configuration() is invoked
  system.log_configuration();

  // Then: at least one new log event is published
  let events = events.lock().clone();
  assert!(
    events[baseline_len..].iter().any(|event| matches!(event, EventStreamEvent::Log(_))),
    "log_configuration() should publish a log event",
  );

  system.terminate().expect("terminate");
}

#[test]
fn log_returns_facade_that_emits_log_events() {
  // Given: a typed actor system with a recording subscriber
  let system = new_test_system();
  let events = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let subscriber = subscriber_handle(RecordingSubscriber::new(events.clone()));
  let _subscription = system.subscribe_event_stream(&subscriber);
  let baseline_len = events.lock().len();

  // When: log() is called and the facade emits a message
  let logger = system.log();
  logger.emit(LogLevel::Info, "typed facade message");

  // Then: a matching log event is published to the event stream
  let events = events.lock().clone();
  assert!(
    events[baseline_len..].iter().any(|event| {
      matches!(
        event,
        EventStreamEvent::Log(log) if log.level() == LogLevel::Info && log.message() == "typed facade message"
      )
    }),
    "log().emit() should publish the emitted message",
  );

  system.terminate().expect("terminate");
}

#[test]
fn get_when_terminated_tracks_same_lifecycle_as_when_terminated() {
  // Given: a typed actor system and both termination signals
  let system = new_test_system();
  let signal = system.when_terminated();
  let alias_signal = system.get_when_terminated();

  // When/Then: neither signal is terminated before termination
  assert!(!signal.is_terminated());
  assert!(!alias_signal.is_terminated());

  // When: the actor system is terminated
  system.terminate().expect("terminate");

  // Then: both signals observe the same terminated state
  assert!(signal.is_terminated());
  assert!(alias_signal.is_terminated());
}

// --- T13: TypedActorSystem parity surface for Phase 2 system endpoints ---

#[test]
fn event_stream_returns_typed_actor_ref_that_publishes_events_to_registered_subscribers() {
  // 前提: 既存 helper で購読者を登録した typed actor system がある
  let system = new_test_system();
  let recorded_events = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let subscriber = subscriber_handle(RecordingSubscriber::new(recorded_events.clone()));
  let _subscription = system.subscribe_event_stream(&subscriber);
  let baseline_len = recorded_events.lock().len();
  let mut event_stream = system.event_stream();
  let published = EventStreamEvent::Log(LogEvent::new(
    LogLevel::Info,
    "phase2-event-stream-publish".into(),
    Duration::from_millis(7),
    None,
    None,
  ));

  // 実行: 公開 event_stream facade を actor ref として使う
  let result = event_stream.try_tell(EventStreamCommand::Publish(published.clone()));

  // 検証: publish command が成功し、購読者がイベントを観測する
  assert!(result.is_ok(), "event_stream should accept publish commands");
  let recorded = recorded_events.lock();
  assert!(
    recorded[baseline_len..]
      .iter()
      .any(|event| { matches!(event, EventStreamEvent::Log(log) if log.message() == "phase2-event-stream-publish") })
  );
  drop(recorded);

  system.terminate().expect("terminate");
}

#[test]
fn event_stream_supports_subscribe_and_unsubscribe_commands() {
  // 前提: typed actor system と actor-ref ベースの購読者がある
  let system = new_test_system();
  let recorded_events = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let collector = ActorRef::new_with_builtin_lock(Pid::new(900, 0), CollectorSender::new(recorded_events.clone()));
  let mut subscribe_stream = system.event_stream();
  let mut publish_stream = system.event_stream();
  let mut unsubscribe_stream = system.event_stream();
  let mut publish_after_unsubscribe_stream = system.event_stream();
  let first = EventStreamEvent::Log(LogEvent::new(
    LogLevel::Info,
    "phase2-first-event".into(),
    Duration::from_millis(8),
    Some(Pid::new(900, 0)),
    None,
  ));
  let second = EventStreamEvent::Log(LogEvent::new(
    LogLevel::Info,
    "phase2-second-event".into(),
    Duration::from_millis(9),
    Some(Pid::new(900, 0)),
    None,
  ));

  // 実行: 公開 event stream command で購読追加後に購読解除する
  let subscribe = subscribe_stream.try_tell(EventStreamCommand::Subscribe { subscriber: collector.clone() });
  let publish_first = publish_stream.try_tell(EventStreamCommand::Publish(first));
  let unsubscribe = unsubscribe_stream.try_tell(EventStreamCommand::Unsubscribe { subscriber: collector.clone() });
  let publish_second = publish_after_unsubscribe_stream.try_tell(EventStreamCommand::Publish(second));

  // 検証: actor-ref 購読者には最初のイベントだけが届く
  assert!(subscribe.is_ok(), "subscribe command should be accepted");
  assert!(publish_first.is_ok(), "first publish command should be accepted");
  assert!(unsubscribe.is_ok(), "unsubscribe command should be accepted");
  assert!(publish_second.is_ok(), "second publish command should be accepted");
  let recorded = recorded_events.lock();
  assert_eq!(
    recorded
      .iter()
      .filter(|event| matches!(event, EventStreamEvent::Log(log) if log.message() == "phase2-first-event"))
      .count(),
    1,
  );
  assert_eq!(
    recorded
      .iter()
      .filter(|event| matches!(event, EventStreamEvent::Log(log) if log.message() == "phase2-second-event"))
      .count(),
    0,
  );

  system.terminate().expect("terminate");
}

#[test]
fn event_stream_subscribe_command_uses_system_scoped_lock_provider_for_actor_subscribers() {
  // 前提: system に束縛された lock provider を持つ typed actor system がある
  let guardian_props = TypedProps::<u32>::from_behavior_factory(Behaviors::ignore);
  let tick_driver = TickDriverConfig::manual(ManualTestDriver::new());
  let (event_stream_subscriber_shared, provider) = CountingSubscriberLockProvider::new();
  let config = ActorSystemConfig::default().with_shared_factory(provider).with_tick_driver(tick_driver);
  let system = TypedActorSystem::<u32>::new_with_config(&guardian_props, &config).expect("system");
  let collector = ActorRef::new_with_builtin_lock(
    Pid::new(902, 0),
    CollectorSender::new(ArcShared::new(SpinSyncMutex::new(Vec::new()))),
  );
  let mut stream = system.event_stream();

  // 実行: 公開 subscribe command だけで actor-ref 購読を登録する
  let subscribe = stream.try_tell(EventStreamCommand::Subscribe { subscriber: collector });

  // 検証: caller 側から provider を渡さなくても、system に束縛された provider が使われる
  assert!(subscribe.is_ok(), "subscribe command should be accepted without an external lock provider");
  assert_eq!(
    event_stream_subscriber_shared.load(Ordering::SeqCst),
    1,
    "typed event stream should materialize actor subscribers via the system-scoped lock provider"
  );

  system.terminate().expect("terminate");
}

#[test]
fn event_stream_subscription_survives_ephemeral_facade_drop_and_shared_unsubscribe_state() {
  // 前提: typed actor system と actor-ref ベースの購読者がある
  let system = new_test_system();
  let recorded_events = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let collector = ActorRef::new_with_builtin_lock(Pid::new(901, 0), CollectorSender::new(recorded_events.clone()));
  let first = EventStreamEvent::Log(LogEvent::new(
    LogLevel::Info,
    "phase2-ephemeral-first-event".into(),
    Duration::from_millis(10),
    Some(Pid::new(901, 0)),
    None,
  ));
  let second = EventStreamEvent::Log(LogEvent::new(
    LogLevel::Info,
    "phase2-ephemeral-second-event".into(),
    Duration::from_millis(11),
    Some(Pid::new(901, 0)),
    None,
  ));

  // 実行: subscribe / publish / unsubscribe を別々の一時 facade で行う
  {
    let mut subscribe_stream = system.event_stream();
    subscribe_stream
      .try_tell(EventStreamCommand::Subscribe { subscriber: collector.clone() })
      .expect("subscribe command should be accepted");
  }
  {
    let mut publish_stream = system.event_stream();
    publish_stream.try_tell(EventStreamCommand::Publish(first)).expect("first publish command should be accepted");
  }
  {
    let mut unsubscribe_stream = system.event_stream();
    unsubscribe_stream
      .try_tell(EventStreamCommand::Unsubscribe { subscriber: collector.clone() })
      .expect("unsubscribe command should be accepted");
  }
  {
    let mut publish_stream = system.event_stream();
    publish_stream.try_tell(EventStreamCommand::Publish(second)).expect("second publish command should be accepted");
  }

  // 検証: 最初の facade を破棄しても購読は有効で、unsubscribe 状態は共有される
  let recorded = recorded_events.lock();
  assert_eq!(
    recorded
      .iter()
      .filter(|event| matches!(event, EventStreamEvent::Log(log) if log.message() == "phase2-ephemeral-first-event"))
      .count(),
    1,
  );
  assert_eq!(
    recorded
      .iter()
      .filter(|event| matches!(event, EventStreamEvent::Log(log) if log.message() == "phase2-ephemeral-second-event"))
      .count(),
    0,
  );

  system.terminate().expect("terminate");
}

#[test]
fn event_stream_from_untyped_wrapper_reuses_shared_subscription_state() {
  // 前提: typed system と、同じ untyped actor system から作った別 wrapper がある
  let system = new_test_system();
  let other_wrapper = TypedActorSystem::<u32>::from_untyped(system.as_untyped().clone());
  let recorded_events = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let collector = ActorRef::new_with_builtin_lock(Pid::new(902, 0), CollectorSender::new(recorded_events.clone()));
  let first = EventStreamEvent::Log(LogEvent::new(
    LogLevel::Info,
    "phase2-from-untyped-first-event".into(),
    Duration::from_millis(12),
    Some(Pid::new(902, 0)),
    None,
  ));
  let second = EventStreamEvent::Log(LogEvent::new(
    LogLevel::Info,
    "phase2-from-untyped-second-event".into(),
    Duration::from_millis(13),
    Some(Pid::new(902, 0)),
    None,
  ));

  // 実行: subscribe / unsubscribe を元 wrapper と from_untyped wrapper でまたいで行う
  {
    let mut subscribe_stream = system.event_stream();
    subscribe_stream
      .try_tell(EventStreamCommand::Subscribe { subscriber: collector.clone() })
      .expect("subscribe command should be accepted");
  }
  {
    let mut publish_stream = other_wrapper.event_stream();
    publish_stream.try_tell(EventStreamCommand::Publish(first)).expect("first publish command should be accepted");
  }
  {
    let mut unsubscribe_stream = other_wrapper.event_stream();
    unsubscribe_stream
      .try_tell(EventStreamCommand::Unsubscribe { subscriber: collector.clone() })
      .expect("unsubscribe command should be accepted");
  }
  {
    let mut publish_stream = system.event_stream();
    publish_stream.try_tell(EventStreamCommand::Publish(second)).expect("second publish command should be accepted");
  }

  // 検証: 両 wrapper が同じ購読状態を共有する
  let recorded = recorded_events.lock();
  assert_eq!(
    recorded
      .iter()
      .filter(|event| matches!(event, EventStreamEvent::Log(log) if log.message() == "phase2-from-untyped-first-event"))
      .count(),
    1,
  );
  assert_eq!(
    recorded
      .iter()
      .filter(
        |event| matches!(event, EventStreamEvent::Log(log) if log.message() == "phase2-from-untyped-second-event")
      )
      .count(),
    0,
  );

  system.terminate().expect("terminate");
}

#[test]
fn dead_letters_returns_typed_actor_ref_that_records_explicit_routing_entries() {
  // 前提: typed actor system と dead-letter snapshot helper がある
  let system = new_test_system();
  let baseline = system.dead_letter_entries().len();
  let mut dead_letters = system.dead_letters::<u32>();

  // 実行: dead-letters facade 経由でメッセージを送る
  let result = dead_letters.try_tell(42_u32);

  // 検証: sink がメッセージを受理し、explicit routing を記録する
  assert!(result.is_ok(), "dead_letters should accept routed messages");
  let entries = system.dead_letter_entries();
  assert_eq!(entries.len(), baseline + 1);
  assert_eq!(entries.last().expect("dead-letter entry").reason(), DeadLetterReason::ExplicitRouting);

  system.terminate().expect("terminate");
}

// --- T14: TypedActorSystem parity surface for implemented Phase 2 endpoints ---

#[test]
fn ignore_ref_accepts_messages_without_recording_dead_letters() {
  // 前提: typed actor system と ignore ref facade がある
  let system = new_test_system();
  let baseline_dead_letters = system.dead_letter_entries().len();
  let mut ignore_ref = system.ignore_ref::<u32>();

  // 実行: ignore ref にメッセージを送る
  let result = ignore_ref.try_tell(123_u32);

  // 検証: メッセージは受理され、dead letter は記録されない
  assert!(result.is_ok(), "ignore_ref should accept messages");
  assert_eq!(system.dead_letter_entries().len(), baseline_dead_letters);

  system.terminate().expect("terminate");
}

#[test]
fn print_tree_contains_bootstrapped_guardians_and_receptionist() {
  // 前提: bootstrap 済みの typed actor system がある
  let system = new_test_system();

  // 実行: actor hierarchy を debug tree として描画する
  let tree = system.print_tree();

  // 検証: 既知の top-level guardian と receptionist が出力に含まれる
  assert!(tree.contains("system"), "print_tree should include system guardian");
  assert!(tree.contains("user"), "print_tree should include user guardian");
  assert!(tree.contains(SYSTEM_RECEPTIONIST_TOP_LEVEL), "print_tree should include receptionist");

  system.terminate().expect("terminate");
}

#[test]
fn system_actor_of_spawns_actor_under_system_guardian() {
  // 前提: typed actor system と system actor 用 typed props がある
  let system = new_test_system();
  let props = TypedProps::<u32>::from_behavior_factory(Behaviors::ignore);
  let system_guardian_pid = system.state().system_guardian_pid().expect("system guardian pid");

  // 実行: system actor を spawn する
  let system_actor = system.system_actor_of(&props, "phase2-system-actor").expect("system actor");

  // 検証: actor は system guardian の子として登録される
  assert!(system.state().child_pids(system_guardian_pid).contains(&system_actor.pid()));
  assert_eq!(system_actor.path().expect("system actor path").to_string(), "/system/phase2-system-actor");

  system.terminate().expect("terminate");
}

#[test]
fn typed_actor_system_new_rejects_empty_guardian_props() {
  // 前提: factory 未設定の empty guardian props
  let guardian_props = TypedProps::<u32>::empty();
  let tick_driver = TickDriverConfig::manual(ManualTestDriver::new());

  // 実行: typed actor system を bootstrap する
  let result = TypedActorSystem::<u32>::new(&guardian_props, tick_driver);

  // 検証: invalid props として明示的に失敗する
  assert!(matches!(result, Err(crate::core::kernel::actor::spawn::SpawnError::InvalidProps(_))));
}

#[test]
fn system_actor_of_rejects_empty_typed_props() {
  // 前提: 起動済みの typed actor system と empty typed props
  let system = new_test_system();
  let props = TypedProps::<u32>::empty().with_dispatcher_same_as_parent().with_tag("phase2-empty-system-actor");

  // 実行: /system 配下への spawn を試みる
  let result = system.system_actor_of(&props, "empty-system-actor");

  // 検証: factory 未設定のままでは invalid props として拒否される
  assert!(matches!(result, Err(crate::core::kernel::actor::spawn::SpawnError::InvalidProps(_))));

  system.terminate().expect("terminate");
}

#[test]
fn print_tree_contains_spawned_system_actor_name() {
  // 前提: system actor を spawn 済みの typed actor system がある
  let system = new_test_system();
  let props = TypedProps::<u32>::from_behavior_factory(Behaviors::ignore);
  let _system_actor = system.system_actor_of(&props, "tree-visible-system-actor").expect("system actor");

  // 実行: actor hierarchy を debug tree として描画する
  let tree = system.print_tree();

  // 検証: spawn 済み system actor 名が tree 出力に含まれる
  assert!(tree.contains("tree-visible-system-actor"));

  system.terminate().expect("terminate");
}
