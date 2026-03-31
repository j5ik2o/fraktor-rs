use crate::core::kernel::event::logging::LogLevel;

use super::LogOptions;

#[test]
fn log_options_defaults_match_debug_logging() {
  let options = LogOptions::default();
  assert!(options.enabled());
  assert_eq!(options.level(), LogLevel::Debug);
  assert_eq!(options.logger_name(), None);
}

#[test]
fn log_options_builder_overrides_fields() {
  let options =
    LogOptions::new().with_enabled(false).with_level(LogLevel::Info).with_logger_name("typed.behaviors");
  assert!(!options.enabled());
  assert_eq!(options.level(), LogLevel::Info);
  assert_eq!(options.logger_name(), Some("typed.behaviors"));
}
