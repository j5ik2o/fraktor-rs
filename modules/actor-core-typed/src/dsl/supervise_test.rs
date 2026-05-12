use core::time::Duration;

use fraktor_actor_core_kernel_rs::{
  actor::{
    error::ActorError,
    supervision::{
      RestartLimit, RestartStatistics, SupervisorDirective, SupervisorStrategy as KernelSupervisorStrategy,
      SupervisorStrategyConfig, SupervisorStrategyKind,
    },
  },
  event::logging::LogLevel,
};

use super::Supervise;
use crate::{SupervisorStrategy as TypedSupervisorStrategy, dsl::Behaviors};

fn resume_strategy() -> KernelSupervisorStrategy {
  KernelSupervisorStrategy::new(
    SupervisorStrategyKind::OneForOne,
    RestartLimit::WithinWindow(10),
    Duration::from_secs(1),
    |_| SupervisorDirective::Resume,
  )
}

fn restart_strategy() -> KernelSupervisorStrategy {
  KernelSupervisorStrategy::new(
    SupervisorStrategyKind::OneForOne,
    RestartLimit::WithinWindow(10),
    Duration::from_secs(1),
    |_| SupervisorDirective::Restart,
  )
}

struct DatabaseError;
struct NetworkError;

#[test]
fn on_failure_of_registers_type_specific_handler() {
  let behavior = Behaviors::stopped::<u32>();
  let supervised =
    Supervise::new(behavior).on_failure_of::<DatabaseError>(resume_strategy()).on_failure(restart_strategy());

  // The composed strategy should pick Resume for DatabaseError-typed errors.
  let config = supervised.supervisor_override().expect("supervisor_override should be set");
  let db_error = ActorError::recoverable_typed::<DatabaseError>("db fail");
  assert_eq!(config.decide(&db_error), SupervisorDirective::Resume);
}

#[test]
fn on_failure_of_falls_back_for_unmatched_type() {
  let behavior = Behaviors::stopped::<u32>();
  let supervised =
    Supervise::new(behavior).on_failure_of::<DatabaseError>(resume_strategy()).on_failure(restart_strategy());

  let config = supervised.supervisor_override().expect("supervisor_override should be set");
  // NetworkError is not registered, so fallback (Restart) should be used.
  let net_error = ActorError::recoverable_typed::<NetworkError>("net fail");
  assert_eq!(config.decide(&net_error), SupervisorDirective::Restart);
}

#[test]
fn on_failure_of_multiple_handlers_selects_correct_one() {
  let stop_strategy = KernelSupervisorStrategy::new(
    SupervisorStrategyKind::OneForOne,
    RestartLimit::WithinWindow(10),
    Duration::from_secs(1),
    |_| SupervisorDirective::Stop,
  );
  let behavior = Behaviors::stopped::<u32>();
  let supervised = Supervise::new(behavior)
    .on_failure_of::<DatabaseError>(resume_strategy())
    .on_failure_of::<NetworkError>(stop_strategy)
    .on_failure(restart_strategy());

  let config = supervised.supervisor_override().expect("supervisor_override should be set");
  let db_error = ActorError::recoverable_typed::<DatabaseError>("db fail");
  assert_eq!(config.decide(&db_error), SupervisorDirective::Resume);

  let net_error = ActorError::recoverable_typed::<NetworkError>("net fail");
  assert_eq!(config.decide(&net_error), SupervisorDirective::Stop);

  // Untyped error uses fallback.
  let generic_error = ActorError::recoverable("generic");
  assert_eq!(config.decide(&generic_error), SupervisorDirective::Restart);
}

#[test]
fn on_failure_without_handlers_sets_strategy_directly() {
  let behavior = Behaviors::stopped::<u32>();
  let supervised = Supervise::new(behavior).on_failure(resume_strategy());

  let config = supervised.supervisor_override().expect("supervisor_override should be set");
  let error = ActorError::recoverable("test");
  assert_eq!(config.decide(&error), SupervisorDirective::Resume);
}

#[test]
fn on_failure_accepts_typed_resume_factory() {
  let behavior = Behaviors::stopped::<u32>();
  let supervised = Supervise::new(behavior).on_failure(TypedSupervisorStrategy::resume());

  let config = supervised.supervisor_override().expect("supervisor_override should be set");
  let error = ActorError::recoverable("test");
  assert_eq!(config.decide(&error), SupervisorDirective::Resume);
}

#[test]
fn on_failure_preserves_restart_wrapper_configuration() {
  let behavior = Behaviors::stopped::<u32>();
  let strategy = TypedSupervisorStrategy::restart()
    .with_unlimited_restarts(Duration::ZERO)
    .with_stop_children(false)
    .with_stash_capacity(32)
    .with_logging_enabled(false)
    .with_log_level(LogLevel::Warn);
  let supervised = Supervise::new(behavior).on_failure(strategy);

  let config = supervised.supervisor_override().expect("supervisor_override should be set");
  let error = ActorError::recoverable("test");

  assert_eq!(config.kind(), SupervisorStrategyKind::OneForOne);
  assert_eq!(config.decide(&error), SupervisorDirective::Restart);
  assert!(!config.stop_children());
  assert_eq!(config.stash_capacity(), 32);
  assert!(!config.logging_enabled());
  assert_eq!(config.effective_log_level(0), LogLevel::Warn);

  let mut statistics = RestartStatistics::new();
  let first = config.handle_failure(&mut statistics, &error, Duration::ZERO);
  let second = config.handle_failure(&mut statistics, &error, Duration::ZERO);

  assert_eq!(first, SupervisorDirective::Restart);
  assert_eq!(second, SupervisorDirective::Restart);
}

#[test]
fn restart_wrapper_converts_into_standard_supervisor_config() {
  let strategy = TypedSupervisorStrategy::restart();
  let config: SupervisorStrategyConfig = strategy.into();

  assert!(matches!(config, SupervisorStrategyConfig::Standard(_)));
}

#[test]
fn on_failure_preserves_backoff_wrapper_configuration() {
  let behavior = Behaviors::stopped::<u32>();
  let strategy =
    TypedSupervisorStrategy::restart_with_backoff(Duration::from_millis(100), Duration::from_secs(10), 0.2)
      .with_reset_backoff_after(Duration::from_secs(30))
      .with_max_restarts(1)
      .with_stop_children(false)
      .with_stash_capacity(64)
      .with_logging_enabled(false)
      .with_log_level(LogLevel::Warn)
      .with_critical_log_level(LogLevel::Error, 2);
  let supervised = Supervise::new(behavior).on_failure(strategy);

  let config = supervised.supervisor_override().expect("supervisor_override should be set");
  let error = ActorError::recoverable("test");
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
