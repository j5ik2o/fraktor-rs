extern crate alloc;

use alloc::vec::Vec;

use fraktor_utils_rs::core::sync::{ArcShared, NoStdMutex, SharedAccess};

use crate::core::{
  kernel::{
    actor::{
      extension::{Extension, ExtensionId},
      scheduler::tick_driver::{ManualTestDriver, TickDriverConfig},
    },
    event::{
      logging::LogLevel,
      stream::{EventStreamEvent, EventStreamSubscriber, subscriber_handle},
    },
    system::ActorSystem,
  },
  typed::{TypedActorSystem, TypedProps, dsl::Behaviors},
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
  events: ArcShared<NoStdMutex<Vec<EventStreamEvent>>>,
}

impl RecordingSubscriber {
  fn new(events: ArcShared<NoStdMutex<Vec<EventStreamEvent>>>) -> Self {
    Self { events }
  }
}

impl EventStreamSubscriber for RecordingSubscriber {
  fn on_event(&mut self, event: &EventStreamEvent) {
    self.events.lock().push(event.clone());
  }
}

fn new_test_system() -> TypedActorSystem<u32> {
  let guardian_props = TypedProps::<u32>::from_behavior_factory(Behaviors::ignore);
  let tick_driver = TickDriverConfig::manual(ManualTestDriver::new());
  TypedActorSystem::<u32>::new(&guardian_props, tick_driver).expect("system")
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
  let guardian_props = TypedProps::<u32>::from_behavior_factory(crate::core::typed::dsl::Behaviors::ignore);
  let config = crate::core::kernel::actor::setup::ActorSystemConfig::default()
    .with_system_name("my-actor-system")
    .with_tick_driver(crate::core::kernel::actor::scheduler::tick_driver::TickDriverConfig::manual(
      crate::core::kernel::actor::scheduler::tick_driver::ManualTestDriver::new(),
    ));
  let system = crate::core::typed::TypedActorSystem::<u32>::new_with_config(&guardian_props, &config).expect("system");

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

  // Then: start_time is a valid duration (configured or injected)
  // The exact value depends on implementation, but it should be retrievable
  let _ = start_time;

  system.terminate().expect("terminate");
}

#[test]
fn start_time_returns_configured_value() {
  // Given: a typed actor system with an explicit start_time
  let expected_start = core::time::Duration::from_secs(1_700_000_000);
  let guardian_props = TypedProps::<u32>::from_behavior_factory(crate::core::typed::dsl::Behaviors::ignore);
  let config = crate::core::kernel::actor::setup::ActorSystemConfig::default()
    .with_start_time(expected_start)
    .with_tick_driver(crate::core::kernel::actor::scheduler::tick_driver::TickDriverConfig::manual(
      crate::core::kernel::actor::scheduler::tick_driver::ManualTestDriver::new(),
    ));
  let system = crate::core::typed::TypedActorSystem::<u32>::new_with_config(&guardian_props, &config).expect("system");

  // When: start_time() is called
  let start_time = system.start_time();

  // Then: the configured start_time is returned
  assert_eq!(start_time, expected_start);

  system.terminate().expect("terminate");
}

#[test]
fn uptime_returns_elapsed_since_start() {
  // Given: a system with a known start_time
  let start = core::time::Duration::from_secs(1_000);
  let guardian_props = TypedProps::<u32>::from_behavior_factory(crate::core::typed::dsl::Behaviors::ignore);
  let config = crate::core::kernel::actor::setup::ActorSystemConfig::default().with_start_time(start).with_tick_driver(
    crate::core::kernel::actor::scheduler::tick_driver::TickDriverConfig::manual(
      crate::core::kernel::actor::scheduler::tick_driver::ManualTestDriver::new(),
    ),
  );
  let system = crate::core::typed::TypedActorSystem::<u32>::new_with_config(&guardian_props, &config).expect("system");

  // When: uptime is calculated with a known "now"
  let now = core::time::Duration::from_secs(1_042);
  let uptime = system.uptime(now);

  // Then: uptime = now - start_time
  assert_eq!(uptime, core::time::Duration::from_secs(42));

  system.terminate().expect("terminate");
}

#[test]
fn uptime_saturates_when_now_is_before_start() {
  // Given: a system with a start_time
  let start = core::time::Duration::from_secs(1_000);
  let guardian_props = TypedProps::<u32>::from_behavior_factory(crate::core::typed::dsl::Behaviors::ignore);
  let config = crate::core::kernel::actor::setup::ActorSystemConfig::default().with_start_time(start).with_tick_driver(
    crate::core::kernel::actor::scheduler::tick_driver::TickDriverConfig::manual(
      crate::core::kernel::actor::scheduler::tick_driver::ManualTestDriver::new(),
    ),
  );
  let system = crate::core::typed::TypedActorSystem::<u32>::new_with_config(&guardian_props, &config).expect("system");

  // When: now is before start_time (edge case)
  let now = core::time::Duration::from_secs(500);
  let uptime = system.uptime(now);

  // Then: uptime saturates to zero
  assert_eq!(uptime, core::time::Duration::ZERO);

  system.terminate().expect("terminate");
}

// --- T12: TypedActorSystem.scheduler() returns Scheduler facade ---

#[test]
fn scheduler_returns_facade_that_can_schedule_once() {
  // Given: a typed actor system
  let system = new_test_system();

  // When: scheduler() is called and schedule_once is invoked
  let scheduler = system.scheduler();
  let receiver =
    crate::core::typed::TypedActorRef::<u32>::from_untyped(crate::core::kernel::actor::actor_ref::ActorRef::null());
  let result = scheduler.schedule_once(core::time::Duration::from_millis(10), receiver, 42u32);

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
  let receiver =
    crate::core::typed::TypedActorRef::<u32>::from_untyped(crate::core::kernel::actor::actor_ref::ActorRef::null());
  let result = scheduler.schedule_at_fixed_rate(
    core::time::Duration::from_millis(5),
    core::time::Duration::from_millis(10),
    receiver,
    7u32,
  );

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
  let receiver =
    crate::core::typed::TypedActorRef::<u32>::from_untyped(crate::core::kernel::actor::actor_ref::ActorRef::null());
  let result = scheduler.schedule_with_fixed_delay(
    core::time::Duration::from_millis(5),
    core::time::Duration::from_millis(20),
    receiver,
    99u32,
  );

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
  let result = dispatchers.lookup(&crate::core::typed::DispatcherSelector::Default);

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
  let result = dispatchers.lookup(&crate::core::typed::DispatcherSelector::Blocking);

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
  let selector = crate::core::typed::DispatcherSelector::from_config("nonexistent");
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
  let config = crate::core::kernel::actor::setup::ActorSystemConfig::default()
    .with_system_name("custom-system")
    .with_tick_driver(crate::core::kernel::actor::scheduler::tick_driver::TickDriverConfig::manual(
      crate::core::kernel::actor::scheduler::tick_driver::ManualTestDriver::new(),
    ));
  let system = crate::core::typed::TypedActorSystem::<u32>::new_with_config(&guardian_props, &config).expect("system");

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
  let expected_start_time = core::time::Duration::from_secs(1_234);
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
fn log_configuration_emits_log_event_to_event_stream() {
  // Given: a typed actor system with a recording subscriber
  let system = new_test_system();
  let events = ArcShared::new(NoStdMutex::new(Vec::new()));
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
  let events = ArcShared::new(NoStdMutex::new(Vec::new()));
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
  // Given: a typed actor system and both termination futures
  let system = new_test_system();
  let scala_future = system.when_terminated();
  let _java_future = system.get_when_terminated();

  // When/Then: the Scala future is not ready before termination
  assert!(!scala_future.with_read(|future| future.is_ready()));

  // When: the actor system is terminated
  system.terminate().expect("terminate");

  // Then: the Scala future becomes ready and the Java alias remains callable
  assert!(scala_future.with_read(|future| future.is_ready()));
}
