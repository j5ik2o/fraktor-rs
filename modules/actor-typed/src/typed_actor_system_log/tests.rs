use alloc::{string::String, vec::Vec};
use core::{
  fmt::{Display, Formatter, Result as FmtResult},
  sync::atomic::{AtomicUsize, Ordering},
};

use fraktor_actor_core_rs::{
  event::{
    logging::{DefaultLoggingFilter, LogLevel},
    stream::{EventStreamEvent, EventStreamSubscription},
  },
  system::ActorSystem,
};
use fraktor_utils_core_rs::core::sync::{ArcShared, SpinSyncMutex};

use super::TypedActorSystemLog;
use crate::test_support::RecordingSubscriber;

// `Display` 実装のたびに counter をインクリメントするヘルパー。
// lazy formatting 契約（level 無効時は `Arguments` を評価しない）を
// 観測可能にするために使用する。
struct CountingDisplay<'a> {
  counter: &'a AtomicUsize,
  payload: &'a str,
}

impl Display for CountingDisplay<'_> {
  fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
    self.counter.fetch_add(1, Ordering::SeqCst);
    f.write_str(self.payload)
  }
}

fn recorded_log_messages(events: &[EventStreamEvent], level: LogLevel) -> Vec<String> {
  events
    .iter()
    .filter_map(|event| match event {
      | EventStreamEvent::Log(log) if log.level() == level => Some(String::from(log.message())),
      | _ => None,
    })
    .collect()
}

// subscriber を system に紐付けて購読を確立する。
// 戻り値の `EventStreamSubscription` は drop 時に購読解除されるため、
// 呼び出し側でテスト関数の終端まで保持する必要がある。
fn new_subscribed_system() -> (ActorSystem, ArcShared<SpinSyncMutex<Vec<EventStreamEvent>>>, EventStreamSubscription) {
  let system = fraktor_actor_adaptor_std_rs::std::system::new_empty_actor_system();
  let events = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let subscriber = crate::test_support::subscriber_handle(RecordingSubscriber::new(events.clone()));
  let subscription = system.event_stream().subscribe(&subscriber);
  (system, events, subscription)
}

#[test]
fn trace_fmt_publishes_event_with_formatted_message() {
  // Given
  let (system, events, _subscription) = new_subscribed_system();
  let log = TypedActorSystemLog::new(system);

  // When
  log.trace_fmt(format_args!("trace {} {}", 1, "msg"));

  // Then
  let recorded = recorded_log_messages(&events.lock().clone(), LogLevel::Trace);
  assert_eq!(recorded, alloc::vec![String::from("trace 1 msg")]);
}

#[test]
fn debug_fmt_publishes_event_with_formatted_message() {
  // Given
  let (system, events, _subscription) = new_subscribed_system();
  let log = TypedActorSystemLog::new(system);

  // When
  log.debug_fmt(format_args!("debug {}-{}", "a", 2));

  // Then
  let recorded = recorded_log_messages(&events.lock().clone(), LogLevel::Debug);
  assert_eq!(recorded, alloc::vec![String::from("debug a-2")]);
}

#[test]
fn info_fmt_publishes_event_with_formatted_message() {
  // Given
  let (system, events, _subscription) = new_subscribed_system();
  let log = TypedActorSystemLog::new(system);

  // When
  log.info_fmt(format_args!("info {}", 42));

  // Then
  let recorded = recorded_log_messages(&events.lock().clone(), LogLevel::Info);
  assert_eq!(recorded, alloc::vec![String::from("info 42")]);
}

#[test]
fn warn_fmt_publishes_event_with_formatted_message() {
  // Given
  let (system, events, _subscription) = new_subscribed_system();
  let log = TypedActorSystemLog::new(system);

  // When
  log.warn_fmt(format_args!("warn {}/{}", 3, 4));

  // Then
  let recorded = recorded_log_messages(&events.lock().clone(), LogLevel::Warn);
  assert_eq!(recorded, alloc::vec![String::from("warn 3/4")]);
}

#[test]
fn error_fmt_publishes_event_with_formatted_message() {
  // Given
  let (system, events, _subscription) = new_subscribed_system();
  let log = TypedActorSystemLog::new(system);

  // When
  log.error_fmt(format_args!("err {}", "boom"));

  // Then
  let recorded = recorded_log_messages(&events.lock().clone(), LogLevel::Error);
  assert_eq!(recorded, alloc::vec![String::from("err boom")]);
}

#[test]
fn fmt_methods_support_multiple_format_arguments() {
  // Given
  let (system, events, _subscription) = new_subscribed_system();
  let log = TypedActorSystemLog::new(system);

  // When
  // Pekko `infoN("{} {} {} {}", a, b, c, d)` 相当を単一の `format_args!` で表現する。
  log.info_fmt(format_args!("{} {} {} {}", 1, 2, 3, 4));

  // Then
  let recorded = recorded_log_messages(&events.lock().clone(), LogLevel::Info);
  assert_eq!(recorded, alloc::vec![String::from("1 2 3 4")]);
}

#[test]
fn is_level_enabled_returns_true_for_all_levels_by_default() {
  // Given
  // フィルタ未設定（default 経路）では全 level が有効であること。
  let system = fraktor_actor_adaptor_std_rs::std::system::new_empty_actor_system();
  let log = TypedActorSystemLog::new(system);

  // When / Then
  assert!(log.is_level_enabled(LogLevel::Trace));
  assert!(log.is_level_enabled(LogLevel::Debug));
  assert!(log.is_level_enabled(LogLevel::Info));
  assert!(log.is_level_enabled(LogLevel::Warn));
  assert!(log.is_level_enabled(LogLevel::Error));
}

#[test]
fn is_level_enabled_respects_configured_minimum_level() {
  // Given
  let system = fraktor_actor_adaptor_std_rs::std::system::new_empty_actor_system();
  system.state().set_logging_filter(DefaultLoggingFilter::new(LogLevel::Warn));
  let log = TypedActorSystemLog::new(system);

  // When / Then
  // threshold 未満は無効、threshold 以上は有効。
  assert!(!log.is_level_enabled(LogLevel::Trace));
  assert!(!log.is_level_enabled(LogLevel::Debug));
  assert!(!log.is_level_enabled(LogLevel::Info));
  assert!(log.is_level_enabled(LogLevel::Warn));
  assert!(log.is_level_enabled(LogLevel::Error));
}

#[test]
fn fmt_does_not_publish_when_level_is_below_filter_threshold() {
  // Given
  let (system, events, _subscription) = new_subscribed_system();
  system.state().set_logging_filter(DefaultLoggingFilter::new(LogLevel::Warn));
  let log = TypedActorSystemLog::new(system);

  // When
  log.trace_fmt(format_args!("filtered trace"));
  log.debug_fmt(format_args!("filtered debug"));
  log.info_fmt(format_args!("filtered info"));
  log.warn_fmt(format_args!("accepted warn"));
  log.error_fmt(format_args!("accepted error"));

  // Then
  let snapshot = events.lock().clone();
  assert!(recorded_log_messages(&snapshot, LogLevel::Trace).is_empty());
  assert!(recorded_log_messages(&snapshot, LogLevel::Debug).is_empty());
  assert!(recorded_log_messages(&snapshot, LogLevel::Info).is_empty());
  assert_eq!(recorded_log_messages(&snapshot, LogLevel::Warn), alloc::vec![String::from("accepted warn")]);
  assert_eq!(recorded_log_messages(&snapshot, LogLevel::Error), alloc::vec![String::from("accepted error")]);
}

#[test]
fn fmt_does_not_evaluate_arguments_when_level_is_disabled() {
  // Given
  // SLF4J / Pekko LoggerOps の lazy formatting 契約:
  // 対象 level が disabled のときは引数の `Display::fmt` を一切呼ばないこと。
  let system = fraktor_actor_adaptor_std_rs::std::system::new_empty_actor_system();
  system.state().set_logging_filter(DefaultLoggingFilter::new(LogLevel::Error));
  let log = TypedActorSystemLog::new(system);

  let counter = AtomicUsize::new(0);
  let payload = "ignored";
  let watched = CountingDisplay { counter: &counter, payload };

  // When
  // Error 未満は disabled。`Display::fmt` が呼ばれてはならない。
  log.trace_fmt(format_args!("{}", watched));
  log.debug_fmt(format_args!("{}", watched));
  log.info_fmt(format_args!("{}", watched));
  log.warn_fmt(format_args!("{}", watched));

  // Then
  assert_eq!(counter.load(Ordering::SeqCst), 0);
}

#[test]
fn fmt_evaluates_arguments_when_level_is_enabled() {
  // Given
  // disabled 側の lazy 契約だけでなく、enabled 側で実際に評価されることも
  // CountingDisplay で観測可能であることを確認する。
  let (system, _events, _subscription) = new_subscribed_system();
  let log = TypedActorSystemLog::new(system);

  let counter = AtomicUsize::new(0);
  let payload = "eval";
  let watched = CountingDisplay { counter: &counter, payload };

  // When
  log.error_fmt(format_args!("{}", watched));

  // Then
  // error_fmt は default filter 下では enabled。少なくとも 1 回評価される。
  assert!(counter.load(Ordering::SeqCst) >= 1);
}
