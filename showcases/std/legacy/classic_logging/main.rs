use std::{boxed::Box, string::String, thread, time::Duration, vec::Vec};

use fraktor_actor_adaptor_std_rs::std::tick_driver::StdTickDriver;
use fraktor_actor_core_rs::{
  actor::{
    Actor, ActorContext,
    error::ActorError,
    messaging::{AnyMessage, AnyMessageView},
    props::Props,
    setup::ActorSystemConfig,
  },
  event::{
    logging::{ActorLogMarker, ActorLogging, DiagnosticActorLogging, LogLevel, LoggingReceive},
    stream::{EventStreamEvent, EventStreamSubscriber, EventStreamSubscriberShared},
  },
  system::ActorSystem,
};
use fraktor_showcases_std::subscribe_kernel_tracing_logger;
use fraktor_utils_core_rs::core::sync::{SharedLock, SpinSyncMutex};

struct Start;

struct RecordingSubscriber {
  events: SharedLock<Vec<EventStreamEvent>>,
}

impl RecordingSubscriber {
  fn new(events: SharedLock<Vec<EventStreamEvent>>) -> Self {
    Self { events }
  }
}

impl EventStreamSubscriber for RecordingSubscriber {
  fn on_event(&mut self, event: &EventStreamEvent) {
    self.events.with_lock(|events| events.push(event.clone()));
  }
}

fn test_subscriber_handle(subscriber: impl EventStreamSubscriber) -> EventStreamSubscriberShared {
  EventStreamSubscriberShared::from_shared_lock(SharedLock::new_with_driver::<SpinSyncMutex<_>>(Box::new(subscriber)))
}

struct LoggingActor;

impl Actor for LoggingActor {
  fn receive(&mut self, ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if message.downcast_ref::<Start>().is_some() {
      ctx.set_logger_name("example.logging.actor");
      let receive_logging = LoggingReceive::with_label("started");
      receive_logging.log(ctx, &"Start", true);

      let mut classic_logging = ActorLogging::new(ctx);
      classic_logging.log().info("classic actor logging facade");

      let mut diagnostic_logging = DiagnosticActorLogging::new(ctx);
      diagnostic_logging.set_marker(ActorLogMarker::dead_letter("Start"));
      diagnostic_logging.insert_mdc("iam", "example.logging.actor");
      diagnostic_logging.log().warn("classic diagnostic logging facade");

      ctx.log(LogLevel::Debug, "classic actor context debug");
    }
    Ok(())
  }
}

fn main() {
  let events = SharedLock::new_with_driver::<SpinSyncMutex<_>>(Vec::new());
  let subscriber = test_subscriber_handle(RecordingSubscriber::new(events.clone()));
  let props = Props::from_fn(|| LoggingActor);
  let system =
    ActorSystem::create_from_props(&props, ActorSystemConfig::new(StdTickDriver::default())).expect("system");
  let _log_subscription = subscribe_kernel_tracing_logger(&system);
  let _subscription = system.event_stream().subscribe(&subscriber);

  system.user_guardian_ref().tell(AnyMessage::new(Start));

  wait_until(|| {
    events.with_lock(|events| {
      let has_info = events
        .iter()
        .any(|event| matches!(event, EventStreamEvent::Log(log) if log.message() == "classic actor logging facade"));
      let has_warning = events.iter().any(|event| {
        matches!(
          event,
          EventStreamEvent::Log(log)
            if log.message() == "classic diagnostic logging facade"
              && log.marker_name() == Some("pekkoDeadLetter")
              && log.marker_properties().get("pekkoMessageClass").map(String::as_str) == Some("Start")
              && log.mdc().get("iam").map(String::as_str) == Some("example.logging.actor")
        )
      });
      let has_debug = events
        .iter()
        .any(|event| matches!(event, EventStreamEvent::Log(log) if log.message() == "classic actor context debug"));
      let has_receive = events
        .iter()
        .any(|event| matches!(event, EventStreamEvent::Log(log) if log.message().contains("received handled message \"Start\"")));
      has_info && has_warning && has_debug && has_receive
    })
  });
  println!("classic_logging captured {} log event(s)", events.with_lock(|events| events.len()));

  system.terminate().expect("terminate");
}

fn wait_until(mut condition: impl FnMut() -> bool) {
  for _ in 0..1_000 {
    if condition() {
      return;
    }
    thread::sleep(Duration::from_millis(1));
  }
  assert!(condition());
}
