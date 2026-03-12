use super::LogOptions;

#[test]
fn log_options_defaults_match_debug_logging() {
  let options = LogOptions::default();
  assert!(options.enabled());
  assert_eq!(options.level(), tracing::Level::DEBUG);
  assert_eq!(options.logger_name(), None);
}

#[test]
fn log_options_builder_overrides_fields() {
  let options =
    LogOptions::new().with_enabled(false).with_level(tracing::Level::INFO).with_logger_name("typed.behaviors");
  assert!(!options.enabled());
  assert_eq!(options.level(), tracing::Level::INFO);
  assert_eq!(options.logger_name(), Some("typed.behaviors"));
}
