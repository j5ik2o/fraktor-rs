use alloc::vec::Vec;

use fraktor_utils_core_rs::core::sync::{ArcShared, SpinSyncMutex};

use super::super::{default_logging_filter::DefaultLoggingFilter, log_event::LogEvent, logging_filter::LoggingFilter};
use crate::core::kernel::{
  actor::Pid,
  event::{
    logging::{ActorLogMarker, LogLevel, LoggingAdapter, tests::RecordingSubscriber},
    stream::{EventStreamEvent, tests::subscriber_handle},
  },
  system::ActorSystem,
};

struct MarkerOnlyFilter;

impl LoggingFilter for MarkerOnlyFilter {
  fn should_publish(&self, event: &LogEvent) -> bool {
    event.marker_name() == Some("pekkoDeadLetter")
  }
}

fn assert_logging_filter(_filter: &impl LoggingFilter) {}

#[test]
fn default_logging_filter_implements_logging_filter() {
  // Given
  let filter = DefaultLoggingFilter::new(LogLevel::Warn);

  // When / Then
  assert_logging_filter(&filter);
}

#[test]
fn logging_adapter_emits_marker_and_mdc_metadata_via_event_stream() {
  let system = ActorSystem::new_empty();
  let events = ArcShared::new(SpinSyncMutex::new(Vec::new()));
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

#[test]
fn logging_adapter_does_not_publish_event_rejected_by_default_filter() {
  // Given
  let system = ActorSystem::new_empty();
  let events = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let subscriber = subscriber_handle(RecordingSubscriber::new(events.clone()));
  let _subscription = system.event_stream().subscribe(&subscriber);
  system.state().set_logging_filter(DefaultLoggingFilter::new(LogLevel::Error));
  let adapter = LoggingAdapter::new(system.clone(), None, Some("classic.logging".into()));

  // When
  adapter.warn("filtered warn");
  adapter.error("accepted error");

  // Then
  let events = events.lock().clone();
  assert!(!events.iter().any(|event| {
    matches!(event, EventStreamEvent::Log(log) if log.level() == LogLevel::Warn && log.message() == "filtered warn")
  }));
  assert!(events.iter().any(|event| {
    matches!(event, EventStreamEvent::Log(log) if log.level() == LogLevel::Error && log.message() == "accepted error")
  }));
}

#[test]
fn logging_adapter_can_publish_only_marked_events_when_filter_requires_marker() {
  // Given
  let system = ActorSystem::new_empty();
  let events = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let subscriber = subscriber_handle(RecordingSubscriber::new(events.clone()));
  let _subscription = system.event_stream().subscribe(&subscriber);
  system.state().set_logging_filter(MarkerOnlyFilter);
  let mut adapter = LoggingAdapter::new(system.clone(), None, Some("classic.logging".into()));

  // When
  adapter.set_marker(ActorLogMarker::dead_letter("MarkedMessage"));
  adapter.warn("marked");
  adapter.clear_marker();
  adapter.warn("plain");

  // Then
  let events = events.lock().clone();
  assert!(events.iter().any(|event| {
    matches!(
      event,
      EventStreamEvent::Log(log)
        if log.message() == "marked"
          && log.marker_name() == Some("pekkoDeadLetter")
    )
  }));
  assert!(!events.iter().any(|event| { matches!(event, EventStreamEvent::Log(log) if log.message() == "plain") }));
}
