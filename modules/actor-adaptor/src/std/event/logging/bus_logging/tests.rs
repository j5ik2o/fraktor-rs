use alloc::vec::Vec;

use fraktor_actor_rs::core::kernel::{
  actor::Pid,
  event::{
    logging::LogLevel,
    stream::{EventStreamEvent, subscriber_handle},
  },
  system::ActorSystem,
};
use fraktor_utils_rs::core::sync::{ArcShared, NoStdMutex};

use super::BusLogging;
use crate::std::event::logging::tests::RecordingSubscriber;

#[test]
fn bus_logging_emits_event_without_actor_context() {
  let system = ActorSystem::new_empty();
  let events = ArcShared::new(NoStdMutex::new(Vec::new()));
  let subscriber = subscriber_handle(RecordingSubscriber::new(events.clone()));
  let _subscription = system.event_stream().subscribe(&subscriber);

  let mut logging = BusLogging::new(system.clone(), Some(Pid::new(77, 0)), Some(String::from("bus.logger")));
  logging.log().warn("bus logging facade");

  let events = events.lock().clone();
  assert!(events.iter().any(|event| {
    matches!(
      event,
      EventStreamEvent::Log(log)
        if log.level() == LogLevel::Warn
          && log.origin() == Some(Pid::new(77, 0))
          && log.logger_name() == Some("bus.logger")
          && log.message() == "bus logging facade"
    )
  }));
}
