use alloc::vec::Vec;
use std::path::Path;

use fraktor_actor_rs::core::kernel::{
  actor::ActorContext,
  event::{
    logging::LogLevel,
    stream::{EventStreamEvent, EventStreamSubscriber, subscriber_handle},
  },
  system::ActorSystem,
};
use fraktor_utils_rs::core::sync::{ArcShared, NoStdMutex};

struct NoopSubscriber;

impl crate::std::event::stream::EventStreamSubscriber for NoopSubscriber {
  fn on_event(&mut self, _event: &EventStreamEvent) {}
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

#[test]
fn std_public_modules_expose_only_live_entry_points() {
  let _behaviors = core::marker::PhantomData::<crate::std::typed::Behaviors>;
  let _log_options = core::marker::PhantomData::<fraktor_actor_rs::core::typed::LogOptions>;
  let _actor_log_marker = core::marker::PhantomData::<crate::std::event::logging::ActorLogMarker>;
  let _actor_logging = core::marker::PhantomData::<crate::std::event::logging::ActorLogging>;
  let _diagnostic_actor_logging = core::marker::PhantomData::<crate::std::event::logging::DiagnosticActorLogging>;
  let _bus_logging = core::marker::PhantomData::<crate::std::event::logging::BusLogging>;
  let _logging_adapter = core::marker::PhantomData::<crate::std::event::logging::LoggingAdapter>;
  let _logging_receive = core::marker::PhantomData::<crate::std::event::logging::LoggingReceive>;
  let _no_logging = core::marker::PhantomData::<crate::std::event::logging::NoLogging>;
  let _tracing_subscriber = core::marker::PhantomData::<crate::std::event::logging::TracingLoggerSubscriber>;
  let _shared = core::marker::PhantomData::<crate::std::event::stream::EventStreamSubscriberShared>;

  let _subscriber = crate::std::event::stream::subscriber_handle(NoopSubscriber);
}

#[test]
fn std_logging_module_exposes_classic_logging_family() {
  let _actor_log_marker = core::marker::PhantomData::<crate::std::event::logging::ActorLogMarker>;
  let _actor_logging = core::marker::PhantomData::<crate::std::event::logging::ActorLogging>;
  let _diagnostic_actor_logging = core::marker::PhantomData::<crate::std::event::logging::DiagnosticActorLogging>;
  let _bus_logging = core::marker::PhantomData::<crate::std::event::logging::BusLogging>;
  let _logging_adapter = core::marker::PhantomData::<crate::std::event::logging::LoggingAdapter>;
  let _logging_receive = core::marker::PhantomData::<crate::std::event::logging::LoggingReceive>;
  let _no_logging = core::marker::PhantomData::<crate::std::event::logging::NoLogging>;
  let _tracing_subscriber = core::marker::PhantomData::<crate::std::event::logging::TracingLoggerSubscriber>;
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
