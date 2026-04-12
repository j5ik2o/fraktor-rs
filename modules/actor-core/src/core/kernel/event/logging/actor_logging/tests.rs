use alloc::vec::Vec;

use fraktor_utils_core_rs::core::sync::{ArcShared, SpinSyncMutex};

use crate::core::kernel::{
  actor::ActorContext,
  event::{
    logging::{ActorLogging, LogLevel, tests::RecordingSubscriber},
    stream::{EventStreamEvent, tests::subscriber_handle},
  },
  system::ActorSystem,
};

#[test]
fn actor_logging_uses_context_pid_and_logger_name() {
  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let events = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let subscriber = subscriber_handle(RecordingSubscriber::new(events.clone()));
  let _subscription = system.event_stream().subscribe(&subscriber);
  let mut context = ActorContext::new(&system, pid);
  context.set_logger_name("classic.actor.logging");

  let mut logging = ActorLogging::new(&context);
  logging.log().info("classic actor logging");

  let events = events.lock().clone();
  assert!(events.iter().any(|event| {
    matches!(
      event,
      EventStreamEvent::Log(log)
        if log.level() == LogLevel::Info
          && log.origin() == Some(pid)
          && log.logger_name() == Some("classic.actor.logging")
          && log.message() == "classic actor logging"
    )
  }));
}
