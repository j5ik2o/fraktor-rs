use alloc::vec::Vec;
use std::path::Path;

use fraktor_actor_rs::core::kernel::{
  actor::ActorContext,
  event::{
    logging::{
      ActorLogMarker, ActorLogging, BusLogging, DiagnosticActorLogging, LogLevel, LoggingAdapter, LoggingReceive,
      NoLogging,
    },
    stream::{EventStreamEvent, EventStreamSubscriber, subscriber_handle},
  },
  system::ActorSystem,
};
use fraktor_utils_rs::core::sync::{ArcShared, NoStdMutex};

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

#[test]
fn std_public_modules_expose_only_live_entry_points() {
  let _log_options = core::marker::PhantomData::<fraktor_actor_rs::core::typed::LogOptions>;
  let _core_actor_log_marker = core::marker::PhantomData::<ActorLogMarker>;
  let _core_actor_logging = core::marker::PhantomData::<ActorLogging>;
  let _core_diagnostic_actor_logging = core::marker::PhantomData::<DiagnosticActorLogging>;
  let _core_bus_logging = core::marker::PhantomData::<BusLogging>;
  let _core_logging_adapter = core::marker::PhantomData::<LoggingAdapter>;
  let _core_logging_receive = core::marker::PhantomData::<LoggingReceive>;
  let _core_no_logging = core::marker::PhantomData::<NoLogging>;
  let _tracing_subscriber = core::marker::PhantomData::<crate::std::event::logging::TracingLoggerSubscriber>;
  let _dead_letter_subscriber = core::marker::PhantomData::<crate::std::event::stream::DeadLetterLogSubscriber>;
  let _std_clock = core::marker::PhantomData::<crate::std::time::StdClock>;
  let _circuit_breaker = core::marker::PhantomData::<crate::std::pattern::CircuitBreaker>;
  let _circuit_breaker_shared = core::marker::PhantomData::<crate::std::pattern::CircuitBreakerShared>;
}

#[test]
fn std_public_source_files_stay_adapter_only() {
  let logging_source = include_str!("event/logging.rs");
  assert!(logging_source.contains("pub use tracing_logger_subscriber::TracingLoggerSubscriber;"));
  assert!(!logging_source.contains("ActorLogMarker"));
  assert!(!logging_source.contains("ActorLogging"));
  assert!(!logging_source.contains("DiagnosticActorLogging"));
  assert!(!logging_source.contains("BusLogging"));
  assert!(!logging_source.contains("LoggingAdapter"));
  assert!(!logging_source.contains("LoggingReceive"));
  assert!(!logging_source.contains("NoLogging"));

  let stream_source = include_str!("event/stream.rs");
  assert!(stream_source.contains("pub use dead_letter_log_subscriber::DeadLetterLogSubscriber;"));
  assert!(!stream_source.contains("EventStreamSubscriberShared"));
  assert!(!stream_source.contains("subscriber_handle"));

  let pattern_source = include_str!("pattern.rs");
  assert!(pattern_source.contains("mod circuit_breaker_bindings;"));
  assert!(pattern_source.contains("pub use circuit_breaker_bindings::{"));
  assert!(!pattern_source.contains("pub fn ask_with_timeout("));
  assert!(!pattern_source.contains("pub async fn graceful_stop("));
  assert!(!pattern_source.contains("pub async fn graceful_stop_with_message("));
  assert!(!pattern_source.contains("pub async fn retry<"));

  let bindings_source = include_str!("pattern/circuit_breaker_bindings.rs");
  assert!(bindings_source.contains("pub type CircuitBreaker ="));
  assert!(bindings_source.contains("pub type CircuitBreakerShared ="));
  assert!(bindings_source.contains("pub fn circuit_breaker("));
  assert!(bindings_source.contains("pub fn circuit_breaker_shared("));

  let time_source = include_str!("time.rs");
  assert!(time_source.contains("pub use std_clock::StdClock;"));
}

#[test]
fn classic_actor_context_log_emits_context_bound_log_event() {
  // Given: event stream を購読した classic actor context がある
  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let events = ArcShared::new(NoStdMutex::new(Vec::new()));
  let subscriber = subscriber_handle(RecordingSubscriber::new(events.clone()));
  let _subscription = system.event_stream().subscribe(&subscriber);
  let mut context = ActorContext::new(&system, pid);
  context.set_logger_name("classic.actor.test");

  // When: classic actor context から直接 log() を呼ぶ
  context.log(LogLevel::Info, "classic context message");

  // Then: actor context の pid/logger_name を持つ LogEvent が publish される
  let events = events.lock().clone();
  assert!(events.iter().any(|event| {
    matches!(
      event,
      EventStreamEvent::Log(log)
        if log.level() == LogLevel::Info
          && log.message() == "classic context message"
          && log.origin() == Some(pid)
          && log.logger_name() == Some("classic.actor.test")
    )
  }));
}

#[test]
fn module_examples_are_moved_to_showcases_std() {
  let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
  let cargo_toml = std::fs::read_to_string(manifest_dir.join("Cargo.toml")).expect("Cargo.toml should be readable");
  let legacy_example = manifest_dir.join("examples/classic_logging.rs");

  assert!(!legacy_example.exists(), "module example must not exist: {}", legacy_example.display());
  assert!(
    !cargo_toml.contains("name = \"classic_logging\""),
    "module Cargo.toml must not define classic_logging example anymore",
  );
}
