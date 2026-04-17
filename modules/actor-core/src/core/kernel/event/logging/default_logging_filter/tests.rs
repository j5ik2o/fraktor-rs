use alloc::{collections::BTreeMap, string::String};
use core::time::Duration;

use super::super::{
  default_logging_filter::DefaultLoggingFilter, log_event::LogEvent, log_level::LogLevel, logging_filter::LoggingFilter,
};

fn make_event(level: LogLevel) -> LogEvent {
  LogEvent::new(level, String::from("test"), Duration::from_secs(1), None, None)
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
fn default_logging_filter_publishes_event_at_threshold() {
  // Given
  let filter = DefaultLoggingFilter::new(LogLevel::Warn);
  let event = make_event(LogLevel::Warn);

  // When / Then
  assert!(filter.should_publish(&event));
}

#[test]
fn default_logging_filter_rejects_event_below_threshold() {
  // Given
  let filter = DefaultLoggingFilter::new(LogLevel::Error);
  let event = make_event(LogLevel::Info);

  // When / Then
  assert!(!filter.should_publish(&event));
}

#[test]
fn default_logging_filter_ignores_marker_metadata_for_level_decision() {
  // Given
  let filter = DefaultLoggingFilter::new(LogLevel::Info);
  let event = make_event(LogLevel::Info).with_marker("pekkoDeadLetter", BTreeMap::new());

  // When / Then
  assert!(filter.should_publish(&event));
}

#[test]
fn default_logging_filter_is_level_enabled_true_at_threshold() {
  // Given
  let filter = DefaultLoggingFilter::new(LogLevel::Warn);

  // When / Then
  // threshold と同じ level は有効であること (>= 比較の境界)。
  assert!(filter.is_level_enabled(LogLevel::Warn));
}

#[test]
fn default_logging_filter_is_level_enabled_true_above_threshold() {
  // Given
  let filter = DefaultLoggingFilter::new(LogLevel::Warn);

  // When / Then
  assert!(filter.is_level_enabled(LogLevel::Error));
}

#[test]
fn default_logging_filter_is_level_enabled_false_below_threshold() {
  // Given
  let filter = DefaultLoggingFilter::new(LogLevel::Warn);

  // When / Then
  // threshold より下の 3 level はすべて無効であること。
  assert!(!filter.is_level_enabled(LogLevel::Trace));
  assert!(!filter.is_level_enabled(LogLevel::Debug));
  assert!(!filter.is_level_enabled(LogLevel::Info));
}

#[test]
fn default_logging_filter_is_level_enabled_accepts_all_when_threshold_is_trace() {
  // Given
  let filter = DefaultLoggingFilter::new(LogLevel::Trace);

  // When / Then
  // Trace を threshold に据えると全 level が有効 = Default 相当の挙動。
  assert!(filter.is_level_enabled(LogLevel::Trace));
  assert!(filter.is_level_enabled(LogLevel::Debug));
  assert!(filter.is_level_enabled(LogLevel::Info));
  assert!(filter.is_level_enabled(LogLevel::Warn));
  assert!(filter.is_level_enabled(LogLevel::Error));
}

#[test]
fn default_logging_filter_is_level_enabled_only_error_when_threshold_is_error() {
  // Given
  let filter = DefaultLoggingFilter::new(LogLevel::Error);

  // When / Then
  // 最高位 Error を threshold にしたとき、有効なのは Error のみ。
  assert!(!filter.is_level_enabled(LogLevel::Trace));
  assert!(!filter.is_level_enabled(LogLevel::Debug));
  assert!(!filter.is_level_enabled(LogLevel::Info));
  assert!(!filter.is_level_enabled(LogLevel::Warn));
  assert!(filter.is_level_enabled(LogLevel::Error));
}
