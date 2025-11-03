use core::time::Duration;

use super::{
  super::{supervisor_directive::SupervisorDirective, supervisor_strategy_kind::SupervisorStrategyKind},
  SupervisorStrategy,
};
use crate::{error::ActorError, supervision::RestartStatistics};

fn restart_only(_error: &ActorError) -> SupervisorDirective {
  SupervisorDirective::Restart
}

fn stop_only(_error: &ActorError) -> SupervisorDirective {
  SupervisorDirective::Stop
}

#[test]
fn restart_within_limit_returns_restart() {
  let mut stats = RestartStatistics::new();
  let strategy = SupervisorStrategy::new(SupervisorStrategyKind::OneForOne, 3, Duration::from_secs(5), restart_only);
  let outcome = strategy.handle_failure(&mut stats, &ActorError::recoverable("fail"), Duration::from_secs(1));
  assert_eq!(outcome, SupervisorDirective::Restart);
  assert_eq!(stats.failure_count(), 1);
}

#[test]
fn exceeding_limit_forces_stop() {
  let mut stats = RestartStatistics::new();
  let strategy = SupervisorStrategy::new(SupervisorStrategyKind::OneForOne, 1, Duration::from_secs(5), restart_only);
  let first = strategy.handle_failure(&mut stats, &ActorError::recoverable("fail"), Duration::from_secs(1));
  let second = strategy.handle_failure(&mut stats, &ActorError::recoverable("fail"), Duration::from_secs(2));
  assert_eq!(first, SupervisorDirective::Restart);
  assert_eq!(second, SupervisorDirective::Stop);
  assert_eq!(stats.failure_count(), 0);
}

#[test]
fn non_restart_resets_statistics() {
  let mut stats = RestartStatistics::new();
  let strategy = SupervisorStrategy::new(SupervisorStrategyKind::OneForOne, 3, Duration::from_secs(5), stop_only);
  stats.record_failure(Duration::from_secs(1), Duration::from_secs(5), Some(3));
  let decision = strategy.handle_failure(&mut stats, &ActorError::recoverable("fail"), Duration::from_secs(2));
  assert_eq!(decision, SupervisorDirective::Stop);
  assert_eq!(stats.failure_count(), 0);
}
