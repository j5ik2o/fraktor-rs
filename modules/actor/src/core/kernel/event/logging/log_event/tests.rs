use alloc::string::String;
use core::time::Duration;

use super::LogEvent;
use crate::core::kernel::{actor::Pid, event::logging::LogLevel};

// --- LogEvent: logger_name field ---

#[test]
fn log_event_new_with_logger_name_stores_value() {
  // Given: a logger name and standard log parameters
  let logger_name = Some(String::from("my.custom.logger"));

  // When: a LogEvent is created with a logger_name
  let event = LogEvent::new(
    LogLevel::Info,
    String::from("test message"),
    Duration::from_millis(100),
    Some(Pid::new(1, 0)),
    logger_name,
  );

  // Then: the logger_name is accessible via the accessor
  assert_eq!(event.logger_name(), Some("my.custom.logger"));
}

#[test]
fn log_event_new_without_logger_name_returns_none() {
  // Given: no logger name specified
  // When: a LogEvent is created without a logger_name
  let event = LogEvent::new(LogLevel::Debug, String::from("debug message"), Duration::from_millis(200), None, None);

  // Then: logger_name returns None
  assert_eq!(event.logger_name(), None);
}

#[test]
fn log_event_preserves_all_fields_with_logger_name() {
  // Given: all fields including logger_name
  let pid = Pid::new(42, 0);
  let logger_name = Some(String::from("actor.context.logger"));

  // When: a LogEvent is created
  let event =
    LogEvent::new(LogLevel::Warn, String::from("warn message"), Duration::from_secs(5), Some(pid), logger_name);

  // Then: all existing fields remain correct alongside logger_name
  assert_eq!(event.level(), LogLevel::Warn);
  assert_eq!(event.message(), "warn message");
  assert_eq!(event.timestamp(), Duration::from_secs(5));
  assert_eq!(event.origin(), Some(pid));
  assert_eq!(event.logger_name(), Some("actor.context.logger"));
}
