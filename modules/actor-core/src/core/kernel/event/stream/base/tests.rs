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
    stream::{ClassifierKey, EventStreamEvent, EventStreamShared, EventStreamSubscriber, tests::subscriber_handle},
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

// ---------------------------------------------------------------------------
// B2: ES-H1 EventStream subchannel classifier
//
// 仕様参照: Pekko `EventBus.scala:136-` (SubchannelClassification) を fraktor の
// closed enum (`EventStreamEvent` 14 variants) 向けに翻訳した版。
// `ClassifierKey` で event variant を識別し、purchaser ごとに購読する variant
// を絞り込む。`ClassifierKey::All` は全 variant を受け取る (= 既存挙動と互換)。
// ---------------------------------------------------------------------------

#[test]
fn es_h1_t1_classifier_key_for_event_returns_matching_key() {
  // Given: 各 EventStreamEvent variant
  let lifecycle = EventStreamEvent::Lifecycle(LifecycleEvent::new(
    Pid::new(1, 0),
    None,
    String::from("actor"),
    LifecycleStage::Started,
    Duration::from_millis(1),
  ));
  let log = EventStreamEvent::Log(LogEvent::new(LogLevel::Info, String::from("msg"), Duration::from_millis(2), None, None));
  let dead_letter = EventStreamEvent::DeadLetter(DeadLetterEntry::new(
    AnyMessage::new(String::from("payload")),
    DeadLetterReason::RecipientUnavailable,
    Some(Pid::new(2, 0)),
    Duration::from_millis(3),
  ));
  let extension = EventStreamEvent::Extension {
    name:    String::from("cluster"),
    payload: AnyMessage::new(String::from("event")),
  };

  // When/Then: ClassifierKey::for_event は variant に対応するキーを返す
  assert_eq!(ClassifierKey::for_event(&lifecycle), ClassifierKey::Lifecycle);
  assert_eq!(ClassifierKey::for_event(&log), ClassifierKey::Log);
  assert_eq!(ClassifierKey::for_event(&dead_letter), ClassifierKey::DeadLetter);
  assert_eq!(ClassifierKey::for_event(&extension), ClassifierKey::Extension);
}

#[test]
fn es_h1_t2_subscribe_with_key_only_receives_matching_events() {
  // Given: Log だけを購読する subscriber
  let stream = EventStreamShared::default();
  let events = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let subscriber = subscriber_handle(RecordingSubscriber::new(events.clone()));
  let _subscription = stream.subscribe_with_key(ClassifierKey::Log, &subscriber);

  // When: Lifecycle と Log を発行
  stream.publish(&EventStreamEvent::Lifecycle(LifecycleEvent::new(
    Pid::new(1, 0),
    None,
    String::from("actor"),
    LifecycleStage::Started,
    Duration::from_millis(1),
  )));
  stream.publish(&EventStreamEvent::Log(LogEvent::new(
    LogLevel::Info,
    String::from("matched"),
    Duration::from_millis(2),
    None,
    None,
  )));
  stream.publish(&EventStreamEvent::Lifecycle(LifecycleEvent::new(
    Pid::new(2, 0),
    None,
    String::from("actor2"),
    LifecycleStage::Stopped,
    Duration::from_millis(3),
  )));

  // Then: Log のみ受け取る (Lifecycle は無視される)
  let recorded = events.lock().clone();
  assert_eq!(recorded.len(), 1);
  assert!(matches!(&recorded[0], EventStreamEvent::Log(event) if event.message() == "matched"));
}

#[test]
fn es_h1_t3_subscribe_with_all_receives_every_event_kind() {
  // Given: ClassifierKey::All で購読
  let stream = EventStreamShared::default();
  let events = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let subscriber = subscriber_handle(RecordingSubscriber::new(events.clone()));
  let _subscription = stream.subscribe_with_key(ClassifierKey::All, &subscriber);

  // When: 異なる variant を順に発行
  stream.publish(&EventStreamEvent::Log(LogEvent::new(
    LogLevel::Info,
    String::from("log"),
    Duration::from_millis(1),
    None,
    None,
  )));
  stream.publish(&EventStreamEvent::Lifecycle(LifecycleEvent::new(
    Pid::new(1, 0),
    None,
    String::from("actor"),
    LifecycleStage::Started,
    Duration::from_millis(2),
  )));
  stream.publish(&EventStreamEvent::Extension {
    name:    String::from("cluster"),
    payload: AnyMessage::new(String::from("ext")),
  });

  // Then: 3 件全て受信
  let recorded = events.lock().clone();
  assert_eq!(recorded.len(), 3);
  assert!(recorded.iter().any(|e| matches!(e, EventStreamEvent::Log(_))));
  assert!(recorded.iter().any(|e| matches!(e, EventStreamEvent::Lifecycle(_))));
  assert!(recorded.iter().any(|e| matches!(e, EventStreamEvent::Extension { .. })));
}

#[test]
fn es_h1_t4_multiple_subscribers_with_different_keys_fan_out_independently() {
  // Given: Lifecycle 専門 / Log 専門 / All の 3 購読者
  let stream = EventStreamShared::default();

  let lifecycle_events = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let lifecycle_subscriber = subscriber_handle(RecordingSubscriber::new(lifecycle_events.clone()));
  let _lifecycle_sub = stream.subscribe_with_key(ClassifierKey::Lifecycle, &lifecycle_subscriber);

  let log_events = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let log_subscriber = subscriber_handle(RecordingSubscriber::new(log_events.clone()));
  let _log_sub = stream.subscribe_with_key(ClassifierKey::Log, &log_subscriber);

  let all_events = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let all_subscriber = subscriber_handle(RecordingSubscriber::new(all_events.clone()));
  let _all_sub = stream.subscribe_with_key(ClassifierKey::All, &all_subscriber);

  // When: Lifecycle / Log / DeadLetter を 1 件ずつ発行
  stream.publish(&EventStreamEvent::Lifecycle(LifecycleEvent::new(
    Pid::new(1, 0),
    None,
    String::from("actor"),
    LifecycleStage::Started,
    Duration::from_millis(1),
  )));
  stream.publish(&EventStreamEvent::Log(LogEvent::new(
    LogLevel::Info,
    String::from("hello"),
    Duration::from_millis(2),
    None,
    None,
  )));
  stream.publish(&EventStreamEvent::DeadLetter(DeadLetterEntry::new(
    AnyMessage::new(String::from("dropped")),
    DeadLetterReason::RecipientUnavailable,
    Some(Pid::new(3, 0)),
    Duration::from_millis(3),
  )));

  // Then:
  // - Lifecycle 購読者: Lifecycle 1 件のみ
  // - Log 購読者: Log 1 件のみ
  // - All 購読者: 3 件全て
  let lifecycle_recorded = lifecycle_events.lock().clone();
  assert_eq!(lifecycle_recorded.len(), 1);
  assert!(matches!(&lifecycle_recorded[0], EventStreamEvent::Lifecycle(_)));

  let log_recorded = log_events.lock().clone();
  assert_eq!(log_recorded.len(), 1);
  assert!(matches!(&log_recorded[0], EventStreamEvent::Log(_)));

  let all_recorded = all_events.lock().clone();
  assert_eq!(all_recorded.len(), 3);
}
