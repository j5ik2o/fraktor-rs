use alloc::vec::Vec;

use fraktor_utils_rs::core::sync::{ArcShared, NoStdMutex};

use crate::core::kernel::{
  actor::Pid,
  event::{
    logging::{ActorLogMarker, LogLevel, LoggingAdapter, tests::RecordingSubscriber},
    stream::{EventStreamEvent, subscriber_handle},
  },
  system::ActorSystem,
};

#[test]
fn logging_adapter_emits_marker_and_mdc_metadata_via_event_stream() {
  let system = ActorSystem::new_empty();
  let events = ArcShared::new(NoStdMutex::new(Vec::new()));
  let subscriber = subscriber_handle(RecordingSubscriber::new(events.clone()));
  let _subscription = system.event_stream().subscribe(&subscriber);
  let pid = Pid::new(42, 0);
  let mut adapter = LoggingAdapter::new(system.clone(), Some(pid), Some("classic.logging".into()));

  adapter.set_marker(ActorLogMarker::dead_letter("ExampleMessage"));
  adapter.insert_mdc("iam", "the one who knocks");

  adapter.warn("classic adapter warning");

  let events = events.lock().clone();
  assert!(events.iter().any(|event| {
    matches!(
      event,
      EventStreamEvent::Log(log)
        if log.level() == LogLevel::Warn
          && log.origin() == Some(pid)
          && log.logger_name() == Some("classic.logging")
          && log.message() == "classic adapter warning"
          && log.marker_name() == Some("pekkoDeadLetter")
          && log.marker_properties().get("pekkoMessageClass").map(String::as_str) == Some("ExampleMessage")
          && log.mdc().get("iam").map(String::as_str) == Some("the one who knocks")
    )
  }));
}
