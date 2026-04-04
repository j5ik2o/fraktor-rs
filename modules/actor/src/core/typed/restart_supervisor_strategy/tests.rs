use core::time::Duration;

use crate::core::{
  kernel::{
    actor::{
      error::ActorError,
      supervision::{SupervisorDirective, SupervisorStrategyConfig, SupervisorStrategyKind},
    },
    event::logging::LogLevel,
  },
  typed::SupervisorStrategy as TypedSupervisorStrategy,
};

#[test]
fn restart_factory_exposes_default_restart_contract() {
  let strategy = TypedSupervisorStrategy::restart();

  assert_eq!(strategy.max_restarts(), 0);
  assert_eq!(strategy.within(), Duration::ZERO);
  assert_eq!(strategy.kind(), SupervisorStrategyKind::OneForOne);
  assert!(strategy.stop_children());
  assert_eq!(strategy.stash_capacity(), usize::MAX);
  assert!(strategy.logging_enabled());
  assert_eq!(strategy.log_level(), LogLevel::Error);
}

#[test]
fn with_limit_maps_negative_one_to_unlimited_without_mutating_source_strategy() {
  let strategy = TypedSupervisorStrategy::restart();
  let configured = strategy.clone().with_limit(-1, Duration::ZERO);

  assert_eq!(strategy.max_restarts(), 0);
  assert_eq!(strategy.within(), Duration::ZERO);
  assert_eq!(configured.max_restarts(), 0);
  assert_eq!(configured.within(), Duration::ZERO);
}

#[test]
#[should_panic(expected = "max_restarts must be -1 or greater")]
fn with_limit_rejects_values_smaller_than_negative_one() {
  let _ = TypedSupervisorStrategy::restart().with_limit(-2, Duration::from_secs(1));
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
