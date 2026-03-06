use core::time::Duration;

use super::BackoffSupervisorStrategy;

#[test]
fn compute_backoff_first_restart() {
  let strategy = BackoffSupervisorStrategy::new(Duration::from_millis(100), Duration::from_secs(10), 0.0);
  let delay = strategy.compute_backoff(0);
  assert_eq!(delay, Duration::from_millis(100));
}

#[test]
fn compute_backoff_exponential_growth() {
  let strategy = BackoffSupervisorStrategy::new(Duration::from_millis(100), Duration::from_secs(10), 0.0);
  assert_eq!(strategy.compute_backoff(1), Duration::from_millis(200));
  assert_eq!(strategy.compute_backoff(2), Duration::from_millis(400));
  assert_eq!(strategy.compute_backoff(3), Duration::from_millis(800));
}

#[test]
fn compute_backoff_caps_at_max() {
  let strategy = BackoffSupervisorStrategy::new(Duration::from_millis(100), Duration::from_millis(500), 0.0);
  assert_eq!(strategy.compute_backoff(3), Duration::from_millis(500));
  assert_eq!(strategy.compute_backoff(10), Duration::from_millis(500));
}

#[test]
fn compute_backoff_large_restart_count_does_not_overflow() {
  let strategy = BackoffSupervisorStrategy::new(Duration::from_millis(100), Duration::from_secs(60), 0.0);
  let delay = strategy.compute_backoff(100);
  assert_eq!(delay, Duration::from_secs(60));
}

#[test]
fn compute_backoff_with_jitter_zero_random() {
  let strategy = BackoffSupervisorStrategy::new(Duration::from_millis(100), Duration::from_secs(10), 0.5);
  let delay = strategy.compute_backoff_with_jitter(0, 0.0);
  assert_eq!(delay, Duration::from_millis(100));
}

#[test]
fn compute_backoff_with_jitter_full_random() {
  let strategy = BackoffSupervisorStrategy::new(Duration::from_millis(100), Duration::from_secs(10), 0.5);
  let delay = strategy.compute_backoff_with_jitter(0, 1.0);
  assert_eq!(delay, Duration::from_millis(150));
}

#[test]
fn compute_backoff_with_jitter_caps_at_max() {
  let strategy = BackoffSupervisorStrategy::new(Duration::from_millis(400), Duration::from_millis(500), 1.0);
  let delay = strategy.compute_backoff_with_jitter(1, 1.0);
  assert_eq!(delay, Duration::from_millis(500));
}

#[test]
fn default_reset_backoff_after() {
  let strategy = BackoffSupervisorStrategy::new(Duration::from_millis(100), Duration::from_secs(10), 0.0);
  let expected = (Duration::from_millis(100) + Duration::from_secs(10)) / 2;
  assert_eq!(strategy.reset_backoff_after(), expected);
}

#[test]
fn default_max_restarts_is_zero() {
  let strategy = BackoffSupervisorStrategy::new(Duration::from_millis(100), Duration::from_secs(10), 0.0);
  assert_eq!(strategy.max_restarts(), 0);
}

#[test]
fn default_stop_children_is_true() {
  let strategy = BackoffSupervisorStrategy::new(Duration::from_millis(100), Duration::from_secs(10), 0.0);
  assert!(strategy.stop_children());
}

#[test]
fn default_stash_capacity_is_1000() {
  let strategy = BackoffSupervisorStrategy::new(Duration::from_millis(100), Duration::from_secs(10), 0.0);
  assert_eq!(strategy.stash_capacity(), 1000);
}

#[test]
fn with_reset_backoff_after_sets_value() {
  let strategy = BackoffSupervisorStrategy::new(Duration::from_millis(100), Duration::from_secs(10), 0.0)
    .with_reset_backoff_after(Duration::from_secs(30));
  assert_eq!(strategy.reset_backoff_after(), Duration::from_secs(30));
}

#[test]
fn with_max_restarts_sets_value() {
  let strategy =
    BackoffSupervisorStrategy::new(Duration::from_millis(100), Duration::from_secs(10), 0.0).with_max_restarts(5);
  assert_eq!(strategy.max_restarts(), 5);
}

#[test]
fn with_stop_children_sets_value() {
  let strategy =
    BackoffSupervisorStrategy::new(Duration::from_millis(100), Duration::from_secs(10), 0.0).with_stop_children(false);
  assert!(!strategy.stop_children());
}

#[test]
fn with_stash_capacity_sets_value() {
  let strategy =
    BackoffSupervisorStrategy::new(Duration::from_millis(100), Duration::from_secs(10), 0.0).with_stash_capacity(500);
  assert_eq!(strategy.stash_capacity(), 500);
}

#[test]
fn accessors_return_constructor_values() {
  let strategy = BackoffSupervisorStrategy::new(Duration::from_millis(200), Duration::from_secs(5), 0.3);
  assert_eq!(strategy.min_backoff(), Duration::from_millis(200));
  assert_eq!(strategy.max_backoff(), Duration::from_secs(5));
  assert!((strategy.random_factor() - 0.3).abs() < f64::EPSILON);
}

#[test]
fn clone_produces_equal_values() {
  let strategy = BackoffSupervisorStrategy::new(Duration::from_millis(100), Duration::from_secs(10), 0.2)
    .with_max_restarts(3)
    .with_stop_children(false)
    .with_stash_capacity(42);
  let cloned = strategy.clone();
  assert_eq!(cloned.min_backoff(), strategy.min_backoff());
  assert_eq!(cloned.max_backoff(), strategy.max_backoff());
  assert!((cloned.random_factor() - strategy.random_factor()).abs() < f64::EPSILON);
  assert_eq!(cloned.reset_backoff_after(), strategy.reset_backoff_after());
  assert_eq!(cloned.max_restarts(), strategy.max_restarts());
  assert!(!cloned.stop_children());
  assert_eq!(cloned.stash_capacity(), 42);
}

#[test]
fn compute_backoff_with_jitter_clamps_nan_random() {
  let strategy = BackoffSupervisorStrategy::new(Duration::from_millis(100), Duration::from_secs(10), 0.5);
  let delay = strategy.compute_backoff_with_jitter(0, f64::NAN);
  // NaN は 0.0 として扱われ、ジッターは適用されない。
  assert_eq!(delay, Duration::from_millis(100));
}

#[test]
fn compute_backoff_with_jitter_clamps_negative_random() {
  let strategy = BackoffSupervisorStrategy::new(Duration::from_millis(100), Duration::from_secs(10), 0.5);
  let delay = strategy.compute_backoff_with_jitter(0, -1.0);
  // 負値は 0.0 にクランプされる。
  assert_eq!(delay, Duration::from_millis(100));
}

#[test]
fn default_logging_enabled_is_true() {
  let strategy = BackoffSupervisorStrategy::new(Duration::from_millis(100), Duration::from_secs(10), 0.0);
  assert!(strategy.logging_enabled());
}

#[test]
fn default_log_level_is_error() {
  use crate::core::event::logging::LogLevel;
  let strategy = BackoffSupervisorStrategy::new(Duration::from_millis(100), Duration::from_secs(10), 0.0);
  assert_eq!(strategy.log_level(), LogLevel::Error);
}

#[test]
fn default_critical_log_level_is_error() {
  use crate::core::event::logging::LogLevel;
  let strategy = BackoffSupervisorStrategy::new(Duration::from_millis(100), Duration::from_secs(10), 0.0);
  assert_eq!(strategy.critical_log_level(), LogLevel::Error);
}

#[test]
fn default_critical_log_level_after_is_zero() {
  let strategy = BackoffSupervisorStrategy::new(Duration::from_millis(100), Duration::from_secs(10), 0.0);
  assert_eq!(strategy.critical_log_level_after(), 0);
}

#[test]
fn with_logging_enabled_false() {
  let strategy = BackoffSupervisorStrategy::new(Duration::from_millis(100), Duration::from_secs(10), 0.0)
    .with_logging_enabled(false);
  assert!(!strategy.logging_enabled());
}

#[test]
fn with_log_level_sets_value() {
  use crate::core::event::logging::LogLevel;
  let strategy = BackoffSupervisorStrategy::new(Duration::from_millis(100), Duration::from_secs(10), 0.0)
    .with_log_level(LogLevel::Warn);
  assert_eq!(strategy.log_level(), LogLevel::Warn);
}

#[test]
fn with_critical_log_level_sets_values() {
  use crate::core::event::logging::LogLevel;
  let strategy = BackoffSupervisorStrategy::new(Duration::from_millis(100), Duration::from_secs(10), 0.0)
    .with_critical_log_level(LogLevel::Warn, 5);
  assert_eq!(strategy.critical_log_level(), LogLevel::Warn);
  assert_eq!(strategy.critical_log_level_after(), 5);
}

#[test]
fn effective_log_level_returns_normal_below_threshold() {
  use crate::core::event::logging::LogLevel;
  let strategy = BackoffSupervisorStrategy::new(Duration::from_millis(100), Duration::from_secs(10), 0.0)
    .with_log_level(LogLevel::Error)
    .with_critical_log_level(LogLevel::Warn, 5);
  assert_eq!(strategy.effective_log_level(0), LogLevel::Error);
  assert_eq!(strategy.effective_log_level(4), LogLevel::Error);
}

#[test]
fn effective_log_level_returns_critical_at_threshold() {
  use crate::core::event::logging::LogLevel;
  let strategy = BackoffSupervisorStrategy::new(Duration::from_millis(100), Duration::from_secs(10), 0.0)
    .with_log_level(LogLevel::Error)
    .with_critical_log_level(LogLevel::Warn, 5);
  assert_eq!(strategy.effective_log_level(5), LogLevel::Warn);
  assert_eq!(strategy.effective_log_level(10), LogLevel::Warn);
}

#[test]
fn effective_log_level_returns_normal_when_threshold_is_zero() {
  use crate::core::event::logging::LogLevel;
  let strategy = BackoffSupervisorStrategy::new(Duration::from_millis(100), Duration::from_secs(10), 0.0)
    .with_log_level(LogLevel::Error)
    .with_critical_log_level(LogLevel::Warn, 0);
  // threshold が 0 の場合は critical log level は使われない。
  assert_eq!(strategy.effective_log_level(0), LogLevel::Error);
  assert_eq!(strategy.effective_log_level(100), LogLevel::Error);
}
