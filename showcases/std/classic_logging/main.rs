#![cfg(not(target_os = "none"))]

use std::{string::String, thread, vec::Vec};

use fraktor_actor_core_rs::core::kernel::{
  actor::{
    Actor, ActorContext,
    error::ActorError,
    messaging::{AnyMessage, AnyMessageView},
    props::Props,
    scheduler::tick_driver::{ManualTestDriver, TickDriverConfig},
  },
  event::{
    logging::{ActorLogMarker, ActorLogging, DiagnosticActorLogging, LogLevel, LoggingReceive},
    stream::{EventStreamEvent, EventStreamSubscriber, subscriber_handle},
  },
  system::ActorSystem,
};
use fraktor_utils_rs::core::sync::{ArcShared, NoStdMutex};

struct Start;

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
  let events = ArcShared::new(NoStdMutex::new(Vec::new()));
  let subscriber = subscriber_handle(RecordingSubscriber::new(events.clone()));
  let props = Props::from_fn(|| LoggingActor);
  let system = ActorSystem::new(&props, TickDriverConfig::manual(ManualTestDriver::new())).expect("system");
  let _subscription = system.event_stream().subscribe(&subscriber);

  system.user_guardian_ref().tell(AnyMessage::new(Start));

  wait_until(|| {
    let events = events.lock();
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
  });

  system.terminate().expect("terminate");
}

fn wait_until(mut condition: impl FnMut() -> bool) {
  for _ in 0..10_000 {
    if condition() {
      return;
    }
    thread::yield_now();
  }
  assert!(condition());
}
