use alloc::{collections::BTreeMap, string::String};
use core::time::Duration;

use super::super::{log_event::LogEvent, log_level::LogLevel, logging_filter::LoggingFilter};

struct MarkerNameFilter {
  marker_name: &'static str,
}

impl MarkerNameFilter {
  const fn new(marker_name: &'static str) -> Self {
    Self { marker_name }
  }
}

impl LoggingFilter for MarkerNameFilter {
  fn should_publish(&self, event: &LogEvent) -> bool {
    event.marker_name() == Some(self.marker_name)
  }
}

/// Filter that relies on the trait's default `is_level_enabled` implementation
/// to verify the contract that implementers opting out of level gating retain
/// the "accept all levels" semantics.
struct AlwaysAcceptFilter;

impl LoggingFilter for AlwaysAcceptFilter {
  fn should_publish(&self, _event: &LogEvent) -> bool {
    true
  }
}

fn assert_logging_filter(_filter: &impl LoggingFilter) {}

#[test]
fn custom_marker_filter_implements_logging_filter() {
  // Given
  let filter = MarkerNameFilter::new("pekkoDeadLetter");

  // When / Then
  assert_logging_filter(&filter);
}

#[test]
fn custom_marker_filter_can_distinguish_marker_presence() {
  // Given
  let filter = MarkerNameFilter::new("pekkoDeadLetter");
  let matching = LogEvent::new(LogLevel::Warn, String::from("matching"), Duration::from_secs(1), None, None)
    .with_marker("pekkoDeadLetter", BTreeMap::new());
  let missing = LogEvent::new(LogLevel::Warn, String::from("missing"), Duration::from_secs(1), None, None);

  // When / Then
  assert!(filter.should_publish(&matching));
  assert!(!filter.should_publish(&missing));
}

#[test]
fn custom_marker_filter_rejects_different_marker_name() {
  // Given
  let filter = MarkerNameFilter::new("pekkoDeadLetter");
  let event = LogEvent::new(LogLevel::Error, String::from("different"), Duration::from_secs(2), None, None)
    .with_marker("pekkoUnhandled", BTreeMap::new());

  // When / Then
  assert!(!filter.should_publish(&event));
}

#[test]
fn default_is_level_enabled_returns_true_for_every_level() {
  // Given
  let filter = AlwaysAcceptFilter;

  // When / Then
  // Trait の default 実装が `true` を返すことを全 level で確認する。
  assert!(filter.is_level_enabled(LogLevel::Trace));
  assert!(filter.is_level_enabled(LogLevel::Debug));
  assert!(filter.is_level_enabled(LogLevel::Info));
  assert!(filter.is_level_enabled(LogLevel::Warn));
  assert!(filter.is_level_enabled(LogLevel::Error));
}
