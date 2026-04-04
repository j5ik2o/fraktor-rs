use core::time::Duration;

use crate::core::{
  kernel::{
    actor::{
      error::ActorError,
      supervision::{RestartStatistics, SupervisorDirective, SupervisorStrategyConfig, SupervisorStrategyKind},
    },
    event::logging::LogLevel,
  },
  typed::SupervisorStrategy as TypedSupervisorStrategy,
};

#[test]
fn restart_with_backoff_factory_exposes_default_backoff_contract() {
  let strategy =
    TypedSupervisorStrategy::restart_with_backoff(Duration::from_millis(100), Duration::from_secs(10), 0.2);

  assert_eq!(strategy.min_backoff(), Duration::from_millis(100));
  assert_eq!(strategy.max_backoff(), Duration::from_secs(10));
  assert!((strategy.random_factor() - 0.2).abs() < f64::EPSILON);
  assert_eq!(strategy.reset_backoff_after(), (Duration::from_millis(100) + Duration::from_secs(10)) / 2);
  assert_eq!(strategy.max_restarts(), 0);
  assert!(strategy.stop_children());
  assert_eq!(strategy.stash_capacity(), usize::MAX);
  assert!(strategy.logging_enabled());
  assert_eq!(strategy.log_level(), LogLevel::Error);
  assert_eq!(strategy.critical_log_level(), LogLevel::Error);
  assert_eq!(strategy.critical_log_level_after(), u32::MAX);
}

#[test]
fn builder_overrides_are_visible_after_conversion_to_supervisor_config() {
  let strategy =
    TypedSupervisorStrategy::restart_with_backoff(Duration::from_millis(100), Duration::from_secs(10), 0.2)
      .with_reset_backoff_after(Duration::from_secs(30))
      .with_max_restarts(1)
      .with_stop_children(false)
      .with_stash_capacity(64)
      .with_logging_enabled(false)
      .with_log_level(LogLevel::Warn)
      .with_critical_log_level(LogLevel::Error, 2);
  let config: SupervisorStrategyConfig = strategy.into();
  let error = ActorError::recoverable("fail");
  let mut statistics = RestartStatistics::new();

  assert_eq!(config.kind(), SupervisorStrategyKind::OneForOne);
  assert_eq!(config.decide(&error), SupervisorDirective::Restart);
  assert!(!config.stop_children());
  assert_eq!(config.stash_capacity(), 64);
  assert!(!config.logging_enabled());
  assert_eq!(config.effective_log_level(1), LogLevel::Warn);
  assert_eq!(config.effective_log_level(2), LogLevel::Error);

  let first = config.handle_failure(&mut statistics, &error, Duration::ZERO);
  let second = config.handle_failure(&mut statistics, &error, Duration::ZERO);

  assert_eq!(first, SupervisorDirective::Restart);
  assert_eq!(second, SupervisorDirective::Stop);
}

#[test]
fn backoff_wrapper_converts_into_backoff_supervisor_config() {
  let strategy =
    TypedSupervisorStrategy::restart_with_backoff(Duration::from_millis(100), Duration::from_secs(10), 0.2);
  let config: SupervisorStrategyConfig = strategy.into();

  assert!(matches!(config, SupervisorStrategyConfig::Backoff(_)));
}
