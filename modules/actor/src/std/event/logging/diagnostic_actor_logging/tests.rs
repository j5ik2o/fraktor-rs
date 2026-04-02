use alloc::vec::Vec;

use fraktor_utils_rs::core::sync::{ArcShared, NoStdMutex};

use super::DiagnosticActorLogging;
use crate::{
  core::kernel::{
    actor::ActorContext,
    event::{
      logging::LogLevel,
      stream::{EventStreamEvent, subscriber_handle},
    },
    system::ActorSystem,
  },
  std::event::logging::{ActorLogMarker, tests::RecordingSubscriber},
};

#[test]
fn diagnostic_actor_logging_emits_marker_and_mdc_metadata() {
  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let events = ArcShared::new(NoStdMutex::new(Vec::new()));
  let subscriber = subscriber_handle(RecordingSubscriber::new(events.clone()));
  let _subscription = system.event_stream().subscribe(&subscriber);
  let mut context = ActorContext::new(&system, pid);
  context.set_logger_name("classic.diagnostic.logging");

  let mut logging = DiagnosticActorLogging::new(&context);
  logging.set_marker(ActorLogMarker::dead_letter("DiagnosticMessage"));
  logging.insert_mdc("iam", "diagnostic");
  logging.log().error("diagnostic actor logging");

  let events = events.lock().clone();
  assert!(events.iter().any(|event| {
    matches!(
      event,
      EventStreamEvent::Log(log)
        if log.level() == LogLevel::Error
          && log.origin() == Some(pid)
          && log.logger_name() == Some("classic.diagnostic.logging")
          && log.message() == "diagnostic actor logging"
          && log.marker_name() == Some("pekkoDeadLetter")
          && log.marker_properties().get("pekkoMessageClass").map(String::as_str) == Some("DiagnosticMessage")
          && log.mdc().get("iam").map(String::as_str) == Some("diagnostic")
    )
  }));
}
