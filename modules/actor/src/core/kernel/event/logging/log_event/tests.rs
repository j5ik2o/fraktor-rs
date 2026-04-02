use alloc::{collections::BTreeMap, string::String};
use core::time::Duration;

use super::LogEvent;
use crate::core::kernel::{actor::Pid, event::logging::LogLevel};

// --- LogEvent: logger_name フィールド ---

#[test]
fn log_event_new_with_logger_name_stores_value() {
  // 前提: logger name と通常のログ引数がある
  let logger_name = Some(String::from("my.custom.logger"));

  // 実行: logger_name 付きで LogEvent を生成する
  let event = LogEvent::new(
    LogLevel::Info,
    String::from("test message"),
    Duration::from_millis(100),
    Some(Pid::new(1, 0)),
    logger_name,
  );

  // 確認: logger_name が accessor から取得できる
  assert_eq!(event.logger_name(), Some("my.custom.logger"));
  assert_eq!(event.marker_name(), None);
  assert!(event.marker_properties().is_empty());
  assert!(event.mdc().is_empty());
}

#[test]
fn log_event_new_without_logger_name_returns_none() {
  // 前提: logger name を指定しない
  // 実行: logger_name なしで LogEvent を生成する
  let event = LogEvent::new(LogLevel::Debug, String::from("debug message"), Duration::from_millis(200), None, None);

  // 確認: logger_name は None を返す
  assert_eq!(event.logger_name(), None);
}

#[test]
fn log_event_preserves_all_fields_with_logger_name() {
  // 前提: logger_name を含む全フィールドを用意する
  let pid = Pid::new(42, 0);
  let logger_name = Some(String::from("actor.context.logger"));

  // 実行: LogEvent を生成する
  let event =
    LogEvent::new(LogLevel::Warn, String::from("warn message"), Duration::from_secs(5), Some(pid), logger_name);

  // 確認: logger_name と既存フィールドの両方が正しく保持される
  assert_eq!(event.level(), LogLevel::Warn);
  assert_eq!(event.message(), "warn message");
  assert_eq!(event.timestamp(), Duration::from_secs(5));
  assert_eq!(event.origin(), Some(pid));
  assert_eq!(event.logger_name(), Some("actor.context.logger"));
}

#[test]
fn log_event_with_marker_and_mdc_preserves_structured_metadata() {
  let marker_properties = BTreeMap::from([(String::from("pekkoMessageClass"), String::from("ExampleMessage"))]);
  let mdc = BTreeMap::from([(String::from("iam"), String::from("the one who knocks"))]);

  let event = LogEvent::new(LogLevel::Error, String::from("structured"), Duration::from_secs(1), None, None)
    .with_marker("pekkoDeadLetter", marker_properties.clone())
    .with_mdc(mdc.clone());

  assert_eq!(event.marker_name(), Some("pekkoDeadLetter"));
  assert_eq!(event.marker_properties(), &marker_properties);
  assert_eq!(event.mdc(), &mdc);
}
