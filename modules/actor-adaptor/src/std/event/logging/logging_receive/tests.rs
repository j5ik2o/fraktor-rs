use alloc::vec::Vec;

use fraktor_actor_rs::core::kernel::{
  actor::ActorContext,
  event::{
    logging::LogLevel,
    stream::{EventStreamEvent, subscriber_handle},
  },
  system::ActorSystem,
};
use fraktor_utils_rs::core::sync::{ArcShared, NoStdMutex};

use super::LoggingReceive;
use crate::std::event::logging::tests::RecordingSubscriber;

#[test]
fn logging_receive_logs_handled_message_with_label() {
  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let events = ArcShared::new(NoStdMutex::new(Vec::new()));
  let subscriber = subscriber_handle(RecordingSubscriber::new(events.clone()));
  let _subscription = system.event_stream().subscribe(&subscriber);
  let mut context = ActorContext::new(&system, pid);
  context.set_logger_name("classic.receive.logging");
  let logging_receive = LoggingReceive::with_label("warmup");

  logging_receive.log(&context, &"ping", true);

  let events = events.lock().clone();
  assert!(events.iter().any(|event| {
    matches!(
      event,
      EventStreamEvent::Log(log)
        if log.level() == LogLevel::Debug
          && log.origin() == Some(pid)
          && log.logger_name() == Some("classic.receive.logging")
          && log.message().contains("received handled message \"ping\" from noSender in state warmup")
    )
  }));
}
