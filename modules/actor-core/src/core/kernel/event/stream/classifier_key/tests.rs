extern crate alloc;

use alloc::string::String;
use core::time::Duration;

use fraktor_utils_core_rs::core::time::TimerInstant;

use super::ClassifierKey;
use crate::core::kernel::{
  actor::{
    Pid,
    actor_ref::dead_letter::{DeadLetterEntry, DeadLetterReason},
    lifecycle::{LifecycleEvent, LifecycleStage},
    messaging::AnyMessage,
    scheduler::tick_driver::{
      AutoDriverMetadata, AutoProfileKind, SchedulerTickMetrics, TickDriverId, TickDriverKind, TickDriverMetadata,
    },
  },
  dispatch::mailbox::metrics_event::{MailboxMetricsEvent, MailboxPressureEvent},
  event::{
    logging::{LogEvent, LogLevel},
    stream::{
      AdapterFailureEvent, BackpressureSignal, CorrelationId, EventStreamEvent, RemoteAuthorityEvent,
      RemotingBackpressureEvent, RemotingLifecycleEvent, TickDriverSnapshot, UnhandledMessageEvent,
    },
  },
  serialization::{SerializationErrorEvent, SerializerId},
  system::state::AuthorityState,
};

#[test]
fn classifier_key_for_event_maps_all_variants() {
  let lifecycle = EventStreamEvent::Lifecycle(LifecycleEvent::new(
    Pid::new(1, 0),
    None,
    String::from("actor"),
    LifecycleStage::Started,
    Duration::from_millis(1),
  ));
  assert_eq!(ClassifierKey::for_event(&lifecycle), ClassifierKey::Lifecycle);

  let log =
    EventStreamEvent::Log(LogEvent::new(LogLevel::Info, String::from("log"), Duration::from_millis(2), None, None));
  assert_eq!(ClassifierKey::for_event(&log), ClassifierKey::Log);

  let dead_letter = EventStreamEvent::DeadLetter(DeadLetterEntry::new(
    AnyMessage::new(String::from("payload")),
    DeadLetterReason::RecipientUnavailable,
    Some(Pid::new(2, 0)),
    Duration::from_millis(3),
  ));
  assert_eq!(ClassifierKey::for_event(&dead_letter), ClassifierKey::DeadLetter);

  let extension = EventStreamEvent::Extension {
    name:    String::from("cluster"),
    payload: AnyMessage::new(String::from("extension")),
  };
  assert_eq!(ClassifierKey::for_event(&extension), ClassifierKey::Extension);

  let mailbox = EventStreamEvent::Mailbox(MailboxMetricsEvent::new(
    Pid::new(3, 0),
    1,
    0,
    Some(16),
    Some(8),
    Duration::from_millis(4),
  ));
  assert_eq!(ClassifierKey::for_event(&mailbox), ClassifierKey::Mailbox);

  let mailbox_pressure = EventStreamEvent::MailboxPressure(MailboxPressureEvent::new(
    Pid::new(4, 0),
    9,
    10,
    90,
    Duration::from_millis(5),
    Some(8),
  ));
  assert_eq!(ClassifierKey::for_event(&mailbox_pressure), ClassifierKey::MailboxPressure);

  let unhandled = EventStreamEvent::UnhandledMessage(UnhandledMessageEvent::new(
    Pid::new(5, 0),
    String::from("probe::Command"),
    Duration::from_millis(6),
  ));
  assert_eq!(ClassifierKey::for_event(&unhandled), ClassifierKey::UnhandledMessage);

  let adapter_failure =
    EventStreamEvent::AdapterFailure(AdapterFailureEvent::custom(Pid::new(6, 0), String::from("boom")));
  assert_eq!(ClassifierKey::for_event(&adapter_failure), ClassifierKey::AdapterFailure);

  let serialization = EventStreamEvent::Serialization(SerializationErrorEvent::new(
    "payload",
    Some(SerializerId::from_raw(200)),
    Some(String::from("manifest")),
    Some(Pid::new(7, 0)),
    Some(String::from("fraktor://sys@host")),
  ));
  assert_eq!(ClassifierKey::for_event(&serialization), ClassifierKey::Serialization);

  let remote_authority =
    EventStreamEvent::RemoteAuthority(RemoteAuthorityEvent::new("remote:2552", AuthorityState::Connected));
  assert_eq!(ClassifierKey::for_event(&remote_authority), ClassifierKey::RemoteAuthority);

  let remoting_backpressure = EventStreamEvent::RemotingBackpressure(RemotingBackpressureEvent::new(
    "remote:2552",
    BackpressureSignal::Apply,
    CorrelationId::new(1, 2),
  ));
  assert_eq!(ClassifierKey::for_event(&remoting_backpressure), ClassifierKey::RemotingBackpressure);

  let remoting_lifecycle = EventStreamEvent::RemotingLifecycle(RemotingLifecycleEvent::Started);
  assert_eq!(ClassifierKey::for_event(&remoting_lifecycle), ClassifierKey::RemotingLifecycle);

  let scheduler_tick =
    EventStreamEvent::SchedulerTick(SchedulerTickMetrics::new(TickDriverKind::Manual, 10, None, 32, 1));
  assert_eq!(ClassifierKey::for_event(&scheduler_tick), ClassifierKey::SchedulerTick);

  let metadata = TickDriverMetadata::new(TickDriverId::new(42), TimerInstant::from_ticks(1, Duration::from_millis(10)));
  let auto = AutoDriverMetadata {
    profile:    AutoProfileKind::Tokio,
    driver_id:  TickDriverId::new(42),
    resolution: Duration::from_millis(10),
  };
  let tick_driver = EventStreamEvent::TickDriver(TickDriverSnapshot::new(
    metadata,
    TickDriverKind::Auto,
    Duration::from_millis(10),
    Some(auto),
  ));
  assert_eq!(ClassifierKey::for_event(&tick_driver), ClassifierKey::TickDriver);
}
