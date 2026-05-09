use core::time::Duration;

use fraktor_actor_core_rs::{
  actor::{
    error::ActorError,
    supervision::{RestartLimit, SupervisorDirective, SupervisorStrategyConfig, SupervisorStrategyKind},
  },
  event::logging::LogLevel,
};

use super::SupervisorStrategy;
use crate::{BackoffSupervisorStrategy, RestartSupervisorStrategy};

#[test]
fn resume_factory_returns_standard_strategy_with_resume_directive() {
  let strategy = SupervisorStrategy::resume();
  let error = ActorError::recoverable("resume");
  let config: SupervisorStrategyConfig = strategy.clone().into();

  assert_eq!(strategy.kind(), SupervisorStrategyKind::OneForOne);
  assert_eq!(strategy.decide(&error), SupervisorDirective::Resume);
  assert!(strategy.logging_enabled());
  assert_eq!(strategy.log_level(), LogLevel::Error);
  assert!(matches!(config, SupervisorStrategyConfig::Standard(_)));
}

#[test]
fn stop_factory_returns_standard_strategy_with_stop_directive() {
  let strategy = SupervisorStrategy::stop();
  let error = ActorError::recoverable("stop");
  let config: SupervisorStrategyConfig = strategy.clone().into();

  assert_eq!(strategy.kind(), SupervisorStrategyKind::OneForOne);
  assert_eq!(strategy.decide(&error), SupervisorDirective::Stop);
  assert!(strategy.logging_enabled());
  assert_eq!(strategy.log_level(), LogLevel::Error);
  assert!(matches!(config, SupervisorStrategyConfig::Standard(_)));
}

#[test]
fn restart_factory_returns_restart_supervisor_strategy_with_default_settings() {
  let strategy: RestartSupervisorStrategy = SupervisorStrategy::restart();

  // SP-M1: typed `restart()` default now mirrors Pekko
  // `Restart(maxRestarts = -1, withinTimeRange = Duration.Zero)`.
  // Previously `max_restarts == 0` encoded "unlimited"; the new encoding
  // makes that explicit via `RestartLimit::Unlimited`.
  assert_eq!(strategy.kind(), SupervisorStrategyKind::OneForOne);
  assert_eq!(strategy.max_restarts(), RestartLimit::Unlimited);
  assert_eq!(strategy.within(), Duration::ZERO);
}

#[test]
fn restart_with_backoff_factory_returns_backoff_supervisor_strategy_with_constructor_values() {
  let strategy: BackoffSupervisorStrategy =
    SupervisorStrategy::restart_with_backoff(Duration::from_millis(100), Duration::from_secs(10), 0.2);

  assert_eq!(strategy.min_backoff(), Duration::from_millis(100));
  assert_eq!(strategy.max_backoff(), Duration::from_secs(10));
  assert!((strategy.random_factor() - 0.2).abs() < f64::EPSILON);
  assert_eq!(strategy.stash_capacity(), usize::MAX);
  assert_eq!(strategy.critical_log_level_after(), u32::MAX);
}

#[test]
fn resume_and_stop_allow_typed_logging_overrides() {
  let resume = SupervisorStrategy::resume().with_logging_enabled(false).with_log_level(LogLevel::Warn);
  let stop = SupervisorStrategy::stop().with_logging_enabled(false).with_log_level(LogLevel::Warn);

  assert!(!resume.logging_enabled());
  assert_eq!(resume.log_level(), LogLevel::Warn);
  assert!(!stop.logging_enabled());
  assert_eq!(stop.log_level(), LogLevel::Warn);
}
