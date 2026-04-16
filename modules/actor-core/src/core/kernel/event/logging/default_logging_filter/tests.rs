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
