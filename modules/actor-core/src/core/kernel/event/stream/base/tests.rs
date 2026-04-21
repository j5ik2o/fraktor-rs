extern crate alloc;

use alloc::{string::String, vec::Vec};
use core::time::Duration;

use fraktor_utils_core_rs::core::sync::{ArcShared, SpinSyncMutex};

use crate::core::kernel::{
  actor::{
    Pid,
    actor_ref::dead_letter::{DeadLetterEntry, DeadLetterReason},
    lifecycle::{LifecycleEvent, LifecycleStage},
    messaging::AnyMessage,
  },
  event::{
    logging::{LogEvent, LogLevel},
    stream::{
      ClassifierKey, EventStream, EventStreamEvent, EventStreamShared, EventStreamSubscriber, tests::subscriber_handle,
    },
  },
};

struct RecordingSubscriber {
  events: ArcShared<SpinSyncMutex<Vec<EventStreamEvent>>>,
}

impl RecordingSubscriber {
  fn new(events: ArcShared<SpinSyncMutex<Vec<EventStreamEvent>>>) -> Self {
    Self { events }
  }
}

impl EventStreamSubscriber for RecordingSubscriber {
  fn on_event(&mut self, event: &EventStreamEvent) {
    self.events.lock().push(event.clone());
  }
}

fn lifecycle_event(actor: &str, timestamp_millis: u64) -> LifecycleEvent {
  LifecycleEvent::new(
    Pid::new(1, 0),
    None,
    String::from(actor),
    LifecycleStage::Started,
    Duration::from_millis(timestamp_millis),
  )
}

fn log_event(message: &str, timestamp_millis: u64) -> LogEvent {
  LogEvent::new(LogLevel::Info, String::from(message), Duration::from_millis(timestamp_millis), None, None)
}

fn dead_letter_event(payload: &str, timestamp_millis: u64) -> DeadLetterEntry {
  DeadLetterEntry::new(
    AnyMessage::new(String::from(payload)),
    DeadLetterReason::RecipientUnavailable,
    Some(Pid::new(9, 0)),
    Duration::from_millis(timestamp_millis),
  )
}

#[test]
fn event_stream_replays_buffer_for_new_subscribers() {
  let stream = EventStreamShared::default();

  let log = LogEvent::new(LogLevel::Info, String::from("boot"), Duration::from_millis(1), None, None);
  stream.publish(&EventStreamEvent::Log(log));

  let events = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let subscriber = subscriber_handle(RecordingSubscriber::new(events.clone()));
  let _subscription = stream.subscribe(&subscriber);

  let lifecycle =
    LifecycleEvent::new(Pid::new(1, 0), None, String::from("actor"), LifecycleStage::Started, Duration::from_millis(2));
  stream.publish(&EventStreamEvent::Lifecycle(lifecycle));

  let events = events.lock().clone();
  assert!(events.iter().any(|event| matches!(event, EventStreamEvent::Log(_))));
  assert!(events.iter().any(|event| matches!(event, EventStreamEvent::Lifecycle(_))));
}

#[test]
fn subscribe_no_replay_skips_buffered_events() {
  let stream = EventStreamShared::default();

  stream.publish(&EventStreamEvent::Log(LogEvent::new(
    LogLevel::Info,
    String::from("buffered"),
    Duration::from_millis(1),
    None,
    None,
  )));

  let events = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let subscriber = subscriber_handle(RecordingSubscriber::new(events.clone()));
  let _subscription = stream.subscribe_no_replay(&subscriber);

  assert!(events.lock().is_empty());

  stream.publish(&EventStreamEvent::Log(LogEvent::new(
    LogLevel::Info,
    String::from("live"),
    Duration::from_millis(2),
    None,
    None,
  )));

  let recorded = events.lock().clone();
  assert_eq!(recorded.len(), 1);
  assert!(matches!(&recorded[0], EventStreamEvent::Log(event) if event.message() == "live"));
}

#[test]
fn capacity_limits_buffer_size() {
  let stream = EventStreamShared::with_capacity(1);

  stream.publish(&EventStreamEvent::Log(LogEvent::new(
    LogLevel::Info,
    String::from("first"),
    Duration::from_millis(1),
    None,
    None,
  )));
  stream.publish(&EventStreamEvent::Log(LogEvent::new(
    LogLevel::Info,
    String::from("second"),
    Duration::from_millis(2),
    None,
    None,
  )));

  let events = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let subscriber = subscriber_handle(RecordingSubscriber::new(events.clone()));
  let _subscription = stream.subscribe(&subscriber);

  let events = events.lock().clone();
  assert_eq!(events.len(), 1);
  assert!(matches!(&events[0], EventStreamEvent::Log(event) if event.message() == "second"));
}

#[test]
fn extension_events_are_buffered_and_delivered() {
  let stream = EventStreamShared::with_capacity(4);

  // publish before subscription to ensure replay works
  stream.publish(&EventStreamEvent::Extension {
    name:    String::from("cluster"),
    payload: AnyMessage::new(String::from("startup")),
  });

  let events = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let subscriber = subscriber_handle(RecordingSubscriber::new(events.clone()));
  let _subscription = stream.subscribe(&subscriber);

  stream.publish(&EventStreamEvent::Extension {
    name:    String::from("cluster"),
    payload: AnyMessage::new(String::from("shutdown")),
  });

  let events = events.lock().clone();
  assert_eq!(events.len(), 2);
  assert!(events.iter().any(|event| match event {
    | EventStreamEvent::Extension { name, payload } => {
      name == "cluster" && payload.payload().downcast_ref::<String>().map(|s| s == "startup").unwrap_or(false)
    },
    | _ => false,
  }));
  assert!(events.iter().any(|event| match event {
    | EventStreamEvent::Extension { name, payload } => {
      name == "cluster" && payload.payload().downcast_ref::<String>().map(|s| s == "shutdown").unwrap_or(false)
    },
    | _ => false,
  }));
}

#[test]
fn unsubscribe_removes_subscriber() {
  let stream = EventStreamShared::default();
  let events = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let subscriber = subscriber_handle(RecordingSubscriber::new(events.clone()));
  let subscription = stream.subscribe(&subscriber);

  stream.publish(&EventStreamEvent::Log(LogEvent::new(
    LogLevel::Info,
    String::from("before unsubscribe"),
    Duration::from_millis(1),
    None,
    None,
  )));

  stream.unsubscribe(subscription.id());

  stream.publish(&EventStreamEvent::Log(LogEvent::new(
    LogLevel::Info,
    String::from("after unsubscribe"),
    Duration::from_millis(2),
    None,
    None,
  )));

  let events = events.lock().clone();
  assert!(
    events.iter().any(|event| matches!(event, EventStreamEvent::Log(event) if event.message() == "before unsubscribe"))
  );
  assert!(
    !events.iter().any(|event| matches!(event, EventStreamEvent::Log(event) if event.message() == "after unsubscribe"))
  );
}

#[test]
fn default_creates_stream_with_default_capacity() {
  let stream = EventStreamShared::default();
  let _ = stream;
}

#[test]
fn with_capacity_creates_stream_with_specified_capacity() {
  let stream = EventStreamShared::with_capacity(100);
  let _ = stream;
}

#[test]
fn es_h1_t1_concrete_key_receives_only_matching_events() {
  let stream = EventStreamShared::default();
  let events = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let subscriber = subscriber_handle(RecordingSubscriber::new(events.clone()));
  let _subscription = stream.subscribe_with_key(ClassifierKey::Log, &subscriber);

  stream.publish(&EventStreamEvent::Lifecycle(lifecycle_event("lifecycle-only", 1)));
  stream.publish(&EventStreamEvent::Log(log_event("matched-log", 2)));
  stream.publish(&EventStreamEvent::DeadLetter(dead_letter_event("dead-letter", 3)));

  let recorded = events.lock().clone();
  assert_eq!(recorded.len(), 1);
  assert!(matches!(&recorded[0], EventStreamEvent::Log(event) if event.message() == "matched-log"));
}

#[test]
fn es_h1_t2_all_key_receives_all_variants() {
  let stream = EventStreamShared::default();
  let events = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let subscriber = subscriber_handle(RecordingSubscriber::new(events.clone()));
  let _subscription = stream.subscribe_with_key(ClassifierKey::All, &subscriber);

  stream.publish(&EventStreamEvent::Log(log_event("all-log", 1)));
  stream.publish(&EventStreamEvent::Lifecycle(lifecycle_event("all-lifecycle", 2)));
  stream.publish(&EventStreamEvent::Extension {
    name:    String::from("cluster"),
    payload: AnyMessage::new(String::from("all-extension")),
  });

  let recorded = events.lock().clone();
  assert_eq!(recorded.len(), 3);
  assert!(recorded.iter().any(|event| matches!(event, EventStreamEvent::Log(log) if log.message() == "all-log")));
  assert!(
    recorded
      .iter()
      .any(|event| matches!(event, EventStreamEvent::Lifecycle(lifecycle) if lifecycle.name() == "all-lifecycle"))
  );
  assert!(recorded.iter().any(|event| match event {
    | EventStreamEvent::Extension { name, payload } => {
      name == "cluster"
        && payload.payload().downcast_ref::<String>().map(|value| value == "all-extension").unwrap_or(false)
    },
    | _ => false,
  }));
}

#[test]
fn es_h1_t3_multiple_subscribers_fan_out_independently() {
  let stream = EventStreamShared::default();

  let lifecycle_events = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let lifecycle_subscriber = subscriber_handle(RecordingSubscriber::new(lifecycle_events.clone()));
  let _lifecycle_subscription = stream.subscribe_with_key(ClassifierKey::Lifecycle, &lifecycle_subscriber);

  let log_events = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let log_subscriber = subscriber_handle(RecordingSubscriber::new(log_events.clone()));
  let _log_subscription = stream.subscribe_with_key(ClassifierKey::Log, &log_subscriber);

  let all_events = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let all_subscriber = subscriber_handle(RecordingSubscriber::new(all_events.clone()));
  let _all_subscription = stream.subscribe_with_key(ClassifierKey::All, &all_subscriber);

  stream.publish(&EventStreamEvent::Lifecycle(lifecycle_event("fanout-lifecycle", 1)));
  stream.publish(&EventStreamEvent::Log(log_event("fanout-log", 2)));
  stream.publish(&EventStreamEvent::DeadLetter(dead_letter_event("fanout-dead-letter", 3)));

  let lifecycle_recorded = lifecycle_events.lock().clone();
  assert_eq!(lifecycle_recorded.len(), 1);
  assert!(matches!(&lifecycle_recorded[0], EventStreamEvent::Lifecycle(event) if event.name() == "fanout-lifecycle"));

  let log_recorded = log_events.lock().clone();
  assert_eq!(log_recorded.len(), 1);
  assert!(matches!(&log_recorded[0], EventStreamEvent::Log(event) if event.message() == "fanout-log"));

  let all_recorded = all_events.lock().clone();
  assert_eq!(all_recorded.len(), 3);
}

#[test]
fn es_h1_t4_publish_prepare_filters_subscribers_by_key() {
  let mut stream = EventStream::default();

  let log_subscriber = subscriber_handle(RecordingSubscriber::new(ArcShared::new(SpinSyncMutex::new(Vec::new()))));
  let (log_id, _) = stream.subscribe_with_key(ClassifierKey::Log, log_subscriber);

  let all_subscriber = subscriber_handle(RecordingSubscriber::new(ArcShared::new(SpinSyncMutex::new(Vec::new()))));
  let (all_id, _) = stream.subscribe_with_key(ClassifierKey::All, all_subscriber);

  let log_targets = stream
    .publish_prepare(EventStreamEvent::Log(log_event("publish-prepare-log", 1)))
    .into_iter()
    .map(|entry| entry.id())
    .collect::<Vec<_>>();
  assert_eq!(log_targets.len(), 2);
  assert!(log_targets.contains(&log_id));
  assert!(log_targets.contains(&all_id));

  let lifecycle_targets = stream
    .publish_prepare(EventStreamEvent::Lifecycle(lifecycle_event("publish-prepare-lifecycle", 2)))
    .into_iter()
    .map(|entry| entry.id())
    .collect::<Vec<_>>();
  assert_eq!(lifecycle_targets.len(), 1);
  assert_eq!(lifecycle_targets[0], all_id);
}

#[test]
fn es_h1_t5_replay_filters_buffered_events_by_key() {
  let stream = EventStreamShared::default();

  stream.publish(&EventStreamEvent::Log(log_event("buffered-log", 1)));
  stream.publish(&EventStreamEvent::Lifecycle(lifecycle_event("buffered-lifecycle", 2)));

  let events = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let subscriber = subscriber_handle(RecordingSubscriber::new(events.clone()));
  let _subscription = stream.subscribe_with_key(ClassifierKey::Log, &subscriber);

  let recorded = events.lock().clone();
  assert_eq!(recorded.len(), 1);
  assert!(matches!(&recorded[0], EventStreamEvent::Log(event) if event.message() == "buffered-log"));
}
