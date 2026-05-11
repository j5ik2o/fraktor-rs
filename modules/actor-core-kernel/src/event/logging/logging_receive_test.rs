use alloc::vec::Vec;

use fraktor_utils_core_rs::sync::{ArcShared, SpinSyncMutex};

use crate::{
  actor::ActorContext,
  event::{
    logging::{LogLevel, LoggingReceive, tests::RecordingSubscriber},
    stream::{EventStreamEvent, tests::subscriber_handle},
  },
  system::ActorSystem,
};

#[test]
fn logging_receive_logs_handled_message_with_label() {
  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let events = ArcShared::new(SpinSyncMutex::new(Vec::new()));
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
