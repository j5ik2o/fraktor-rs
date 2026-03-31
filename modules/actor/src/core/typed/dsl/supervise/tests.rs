use core::time::Duration;

use super::Supervise;
use crate::core::{
  kernel::actor::{
    error::ActorError,
    supervision::{SupervisorDirective, SupervisorStrategy, SupervisorStrategyKind},
  },
  typed::dsl::Behaviors,
};

fn resume_strategy() -> SupervisorStrategy {
  SupervisorStrategy::new(SupervisorStrategyKind::OneForOne, 10, Duration::from_secs(1), |_| {
    SupervisorDirective::Resume
  })
}

fn restart_strategy() -> SupervisorStrategy {
  SupervisorStrategy::new(SupervisorStrategyKind::OneForOne, 10, Duration::from_secs(1), |_| {
    SupervisorDirective::Restart
  })
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
  let stop_strategy = SupervisorStrategy::new(SupervisorStrategyKind::OneForOne, 10, Duration::from_secs(1), |_| {
    SupervisorDirective::Stop
  });
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
