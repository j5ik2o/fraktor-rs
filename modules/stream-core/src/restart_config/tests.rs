use crate::{RestartConfig, RestartLogConfig, RestartLogLevel, StreamError};

#[test]
fn restart_config_normalizes_max_backoff() {
  let settings = RestartConfig::new(5, 1, 3);
  assert_eq!(settings.min_backoff_ticks(), 5);
  assert_eq!(settings.max_backoff_ticks(), 5);
}

#[test]
fn restart_config_clamps_random_factor_permille() {
  let settings = RestartConfig::new(1, 8, 3).with_random_factor_permille(1500);
  assert_eq!(settings.random_factor_permille(), 1000);
}

// --- restart_on tests ---

#[test]
fn restart_config_should_restart_returns_true_when_no_predicate_set() {
  // Given: default settings without restart_on predicate
  let settings = RestartConfig::new(1, 8, 3);

  // When: checking if any error should trigger restart
  let error = StreamError::Failed;

  // Then: all errors trigger restart (default behavior)
  assert!(settings.should_restart(&error));
}

#[test]
fn restart_config_with_restart_on_filters_errors() {
  // Given: settings with a predicate that only restarts on BufferOverflow errors
  let settings = RestartConfig::new(1, 8, 3).with_restart_on(|error| matches!(error, StreamError::BufferOverflow));

  // When: checking different error types
  let overflow_error = StreamError::BufferOverflow;
  let other_error = StreamError::InvalidConnection;

  // Then: only BufferOverflow errors trigger restart
  assert!(settings.should_restart(&overflow_error));
  assert!(!settings.should_restart(&other_error));
}

#[test]
fn restart_config_with_restart_on_preserves_other_fields() {
  // Given: settings with various fields configured
  let settings = RestartConfig::new(2, 10, 5).with_random_factor_permille(500).with_restart_on(|_| true);

  // Then: other fields are preserved
  assert_eq!(settings.min_backoff_ticks(), 2);
  assert_eq!(settings.max_backoff_ticks(), 10);
  assert_eq!(settings.max_restarts(), 5);
  assert_eq!(settings.random_factor_permille(), 500);
}

#[test]
fn restart_config_with_restart_on_is_cloneable() {
  // Given: settings with restart_on predicate (contains Arc, not Copy)
  let settings = RestartConfig::new(1, 8, 3).with_restart_on(|_| false);

  // When: cloning the settings
  let cloned = settings.clone();

  // Then: cloned settings produce the same result
  let error = StreamError::Failed;
  assert_eq!(settings.should_restart(&error), cloned.should_restart(&error));
}

// --- log_settings tests ---

#[test]
fn restart_log_config_default_values() {
  // Given: default RestartLogConfig
  let log_settings = RestartLogConfig::default();

  // Then: defaults match Pekko convention
  assert_eq!(log_settings.log_level(), RestartLogLevel::Warning);
  assert_eq!(log_settings.critical_log_level(), RestartLogLevel::Error);
  assert_eq!(log_settings.critical_log_level_after(), usize::MAX);
}

#[test]
fn restart_log_config_with_custom_values() {
  // Given: custom log settings
  let log_settings = RestartLogConfig::new(RestartLogLevel::Debug, RestartLogLevel::Warning, 5);

  // Then: custom values are preserved
  assert_eq!(log_settings.log_level(), RestartLogLevel::Debug);
  assert_eq!(log_settings.critical_log_level(), RestartLogLevel::Warning);
  assert_eq!(log_settings.critical_log_level_after(), 5);
}

#[test]
fn restart_config_default_log_config() {
  // Given: default RestartConfig
  let settings = RestartConfig::new(1, 8, 3);

  // Then: log_settings uses default values
  let log_settings = settings.log_settings();
  assert_eq!(log_settings.log_level(), RestartLogLevel::Warning);
  assert_eq!(log_settings.critical_log_level(), RestartLogLevel::Error);
}

#[test]
fn restart_config_with_log_config_replaces_defaults() {
  // Given: custom log settings
  let custom_log = RestartLogConfig::new(RestartLogLevel::Info, RestartLogLevel::Error, 10);

  // When: applying custom log settings
  let settings = RestartConfig::new(1, 8, 3).with_log_settings(custom_log);

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

// --- Task K: with_min_backoff_ticks tests ---

#[test]
fn restart_config_with_min_backoff_ticks_sets_value_when_below_max() {
  // Given: 初期 min=1, max=8 の設定
  let settings = RestartConfig::new(1, 8, 3);

  // When: 新しい min を max 未満で設定
  let updated = settings.with_min_backoff_ticks(4);

  // Then: min だけが差し替わり、max は維持される
  assert_eq!(updated.min_backoff_ticks(), 4);
  assert_eq!(updated.max_backoff_ticks(), 8);
}

#[test]
fn restart_config_with_min_backoff_ticks_normalizes_when_above_max() {
  // Given: 初期 min=1, max=8 の設定
  let settings = RestartConfig::new(1, 8, 3);

  // When: max を超える min を設定
  let updated = settings.with_min_backoff_ticks(20);

  // Then: 正規化により max が新しい min に揃えられる
  assert_eq!(updated.min_backoff_ticks(), 20);
  assert_eq!(updated.max_backoff_ticks(), 20);
}

// --- Task K: with_max_backoff_ticks tests ---

#[test]
fn restart_config_with_max_backoff_ticks_sets_value_when_above_min() {
  // Given: 初期 min=2, max=5 の設定
  let settings = RestartConfig::new(2, 5, 3);

  // When: 新しい max を min より大きく設定
  let updated = settings.with_max_backoff_ticks(12);

  // Then: max だけが差し替わり、min は維持される
  assert_eq!(updated.min_backoff_ticks(), 2);
  assert_eq!(updated.max_backoff_ticks(), 12);
}

#[test]
fn restart_config_with_max_backoff_ticks_normalizes_when_below_min() {
  // Given: 初期 min=10, max=20 の設定
  let settings = RestartConfig::new(10, 20, 3);

  // When: min を下回る max を設定
  let updated = settings.with_max_backoff_ticks(4);

  // Then: 正規化により max が min に引き上げられる
  assert_eq!(updated.min_backoff_ticks(), 10);
  assert_eq!(updated.max_backoff_ticks(), 10);
}

// --- Task K: with_max_restarts tests ---

#[test]
fn restart_config_with_max_restarts_replaces_count_and_window() {
  // Given: 初期 max_restarts=3, window=u32::MAX の設定
  let settings = RestartConfig::new(1, 8, 3);

  // When: Pekko 融合 setter で count と window を同時差し替え
  let updated = settings.with_max_restarts(7, 120);

  // Then: 両フィールドが同時に更新される
  assert_eq!(updated.max_restarts(), 7);
  assert_eq!(updated.max_restarts_within_ticks(), 120);
}

#[test]
fn restart_config_with_max_restarts_preserves_other_fields() {
  // Given: 他フィールドを設定済みの設定
  let settings = RestartConfig::new(2, 10, 3).with_random_factor_permille(500).with_jitter_seed(42);

  // When: max_restarts を差し替え
  let updated = settings.with_max_restarts(5, 60);

  // Then: min/max_backoff_ticks や random_factor_permille などは保持される
  assert_eq!(updated.min_backoff_ticks(), 2);
  assert_eq!(updated.max_backoff_ticks(), 10);
  assert_eq!(updated.random_factor_permille(), 500);
  assert_eq!(updated.jitter_seed(), 42);
  assert_eq!(updated.max_restarts(), 5);
  assert_eq!(updated.max_restarts_within_ticks(), 60);
}
