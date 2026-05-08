use core::time::Duration;

use fraktor_actor_core_rs::core::kernel::{
  actor::{
    error::ActorError,
    supervision::{RestartLimit, SupervisorDirective, SupervisorStrategyConfig, SupervisorStrategyKind},
  },
  event::logging::LogLevel,
};

use crate::SupervisorStrategy as TypedSupervisorStrategy;

#[test]
fn restart_factory_exposes_default_restart_contract() {
  let strategy = TypedSupervisorStrategy::restart();

  // SP-M1: default mirrors typed Pekko
  // `Restart(maxRestarts = -1, withinTimeRange = Duration.Zero)`.
  assert_eq!(strategy.max_restarts(), RestartLimit::Unlimited);
  assert_eq!(strategy.within(), Duration::ZERO);
  assert_eq!(strategy.kind(), SupervisorStrategyKind::OneForOne);
  assert!(strategy.stop_children());
  assert_eq!(strategy.stash_capacity(), usize::MAX);
  assert!(strategy.logging_enabled());
  assert_eq!(strategy.log_level(), LogLevel::Error);
}

#[test]
fn with_unlimited_restarts_sets_restart_limit_unlimited() {
  let strategy = TypedSupervisorStrategy::restart();
  let configured = strategy.clone().with_unlimited_restarts(Duration::from_secs(1));

  // Original strategy is not mutated by the builder chain.
  assert_eq!(strategy.max_restarts(), RestartLimit::Unlimited);
  assert_eq!(strategy.within(), Duration::ZERO);
  assert_eq!(configured.max_restarts(), RestartLimit::Unlimited);
  assert_eq!(configured.within(), Duration::from_secs(1));
}

#[test]
fn with_limit_zero_is_accepted_as_no_retry() {
  // Pekko `maxNrOfRetries = 0` means "no retry — stop on first failure"
  // and is a valid configuration (not a panic).
  let strategy = TypedSupervisorStrategy::restart().with_limit(0, Duration::from_secs(1));

  assert_eq!(strategy.max_restarts(), RestartLimit::WithinWindow(0));
  assert_eq!(strategy.within(), Duration::from_secs(1));
}

#[test]
fn with_limit_finite_count_is_represented_as_within_window() {
  let strategy = TypedSupervisorStrategy::restart().with_limit(3, Duration::from_secs(5));

  assert_eq!(strategy.max_restarts(), RestartLimit::WithinWindow(3));
  assert_eq!(strategy.within(), Duration::from_secs(5));
}

#[test]
fn builder_overrides_are_visible_after_conversion_to_supervisor_config() {
  let strategy = TypedSupervisorStrategy::restart()
    .with_limit(3, Duration::from_secs(5))
    .with_stop_children(false)
    .with_stash_capacity(64)
    .with_logging_enabled(false)
    .with_log_level(LogLevel::Warn);
  let config: SupervisorStrategyConfig = strategy.into();
  let error = ActorError::recoverable("fail");

  assert_eq!(config.kind(), SupervisorStrategyKind::OneForOne);
  assert_eq!(config.decide(&error), SupervisorDirective::Restart);
  assert!(!config.stop_children());
  assert_eq!(config.stash_capacity(), 64);
  assert!(!config.logging_enabled());
  assert_eq!(config.effective_log_level(0), LogLevel::Warn);
}
