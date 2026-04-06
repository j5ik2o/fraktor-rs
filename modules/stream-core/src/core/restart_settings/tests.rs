use crate::core::{RestartLogLevel, RestartLogSettings, RestartSettings, StreamError};

#[test]
fn restart_settings_normalizes_max_backoff() {
  let settings = RestartSettings::new(5, 1, 3);
  assert_eq!(settings.min_backoff_ticks(), 5);
  assert_eq!(settings.max_backoff_ticks(), 5);
}

#[test]
fn restart_settings_clamps_random_factor_permille() {
  let settings = RestartSettings::new(1, 8, 3).with_random_factor_permille(1500);
  assert_eq!(settings.random_factor_permille(), 1000);
}

// --- restart_on tests ---

#[test]
fn restart_settings_should_restart_returns_true_when_no_predicate_set() {
  // Given: default settings without restart_on predicate
  let settings = RestartSettings::new(1, 8, 3);

  // When: checking if any error should trigger restart
  let error = StreamError::Failed;

  // Then: all errors trigger restart (default behavior)
  assert!(settings.should_restart(&error));
}

#[test]
fn restart_settings_with_restart_on_filters_errors() {
  // Given: settings with a predicate that only restarts on BufferOverflow errors
  let settings = RestartSettings::new(1, 8, 3).with_restart_on(|error| matches!(error, StreamError::BufferOverflow));

  // When: checking different error types
  let overflow_error = StreamError::BufferOverflow;
  let other_error = StreamError::InvalidConnection;

  // Then: only BufferOverflow errors trigger restart
  assert!(settings.should_restart(&overflow_error));
  assert!(!settings.should_restart(&other_error));
}

#[test]
fn restart_settings_with_restart_on_preserves_other_fields() {
  // Given: settings with various fields configured
  let settings = RestartSettings::new(2, 10, 5).with_random_factor_permille(500).with_restart_on(|_| true);

  // Then: other fields are preserved
  assert_eq!(settings.min_backoff_ticks(), 2);
  assert_eq!(settings.max_backoff_ticks(), 10);
  assert_eq!(settings.max_restarts(), 5);
  assert_eq!(settings.random_factor_permille(), 500);
}

#[test]
fn restart_settings_with_restart_on_is_cloneable() {
  // Given: settings with restart_on predicate (contains Arc, not Copy)
  let settings = RestartSettings::new(1, 8, 3).with_restart_on(|_| false);

  // When: cloning the settings
  let cloned = settings.clone();

  // Then: cloned settings produce the same result
  let error = StreamError::Failed;
  assert_eq!(settings.should_restart(&error), cloned.should_restart(&error));
}

// --- log_settings tests ---

#[test]
fn restart_log_settings_default_values() {
  // Given: default RestartLogSettings
  let log_settings = RestartLogSettings::default();

  // Then: defaults match Pekko convention
  assert_eq!(log_settings.log_level(), RestartLogLevel::Warning);
  assert_eq!(log_settings.critical_log_level(), RestartLogLevel::Error);
  assert_eq!(log_settings.critical_log_level_after(), usize::MAX);
}

#[test]
fn restart_log_settings_with_custom_values() {
  // Given: custom log settings
  let log_settings = RestartLogSettings::new(RestartLogLevel::Debug, RestartLogLevel::Warning, 5);

  // Then: custom values are preserved
  assert_eq!(log_settings.log_level(), RestartLogLevel::Debug);
  assert_eq!(log_settings.critical_log_level(), RestartLogLevel::Warning);
  assert_eq!(log_settings.critical_log_level_after(), 5);
}

#[test]
fn restart_settings_default_log_settings() {
  // Given: default RestartSettings
  let settings = RestartSettings::new(1, 8, 3);

  // Then: log_settings uses default values
  let log_settings = settings.log_settings();
  assert_eq!(log_settings.log_level(), RestartLogLevel::Warning);
  assert_eq!(log_settings.critical_log_level(), RestartLogLevel::Error);
}

#[test]
fn restart_settings_with_log_settings_replaces_defaults() {
  // Given: custom log settings
  let custom_log = RestartLogSettings::new(RestartLogLevel::Info, RestartLogLevel::Error, 10);

  // When: applying custom log settings
  let settings = RestartSettings::new(1, 8, 3).with_log_settings(custom_log);

  // Then: custom log settings are used
  assert_eq!(settings.log_settings().log_level(), RestartLogLevel::Info);
  assert_eq!(settings.log_settings().critical_log_level_after(), 10);
}

#[test]
fn restart_log_level_equality() {
  // Given: RestartLogLevel variants
  // Then: equality works correctly
  assert_eq!(RestartLogLevel::Debug, RestartLogLevel::Debug);
  assert_eq!(RestartLogLevel::Info, RestartLogLevel::Info);
  assert_eq!(RestartLogLevel::Warning, RestartLogLevel::Warning);
  assert_eq!(RestartLogLevel::Error, RestartLogLevel::Error);
  assert_ne!(RestartLogLevel::Debug, RestartLogLevel::Error);
}
