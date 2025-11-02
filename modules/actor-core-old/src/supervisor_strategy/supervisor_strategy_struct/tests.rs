use core::time::Duration;

use super::*;
use crate::actor_error::{ActorError, ActorErrorReason};

fn restart_only(_: &ActorError) -> SupervisorDirective {
  SupervisorDirective::Restart
}

fn stop_only(_: &ActorError) -> SupervisorDirective {
  SupervisorDirective::Stop
}

#[test]
fn restart_within_limit_allows_retry() {
  let strategy = SupervisorStrategy::new(SupervisorStrategyKind::OneForOne, 3, Duration::from_secs(5), restart_only);
  let mut stats = RestartStatistics::new();
  let error = ActorError::recoverable(ActorErrorReason::from("err"));

  let outcome = strategy.handle_failure(&mut stats, &error, Duration::from_secs(1));
  assert_eq!(outcome, SupervisorDirective::Restart);
}

#[test]
fn exceeding_limit_transitions_to_stop() {
  let strategy = SupervisorStrategy::new(SupervisorStrategyKind::OneForOne, 1, Duration::from_secs(5), restart_only);
  let mut stats = RestartStatistics::new();
  let error = ActorError::recoverable(ActorErrorReason::from("err"));

  assert_eq!(strategy.handle_failure(&mut stats, &error, Duration::from_secs(1)), SupervisorDirective::Restart);
  let outcome = strategy.handle_failure(&mut stats, &error, Duration::from_secs(2));
  assert_eq!(outcome, SupervisorDirective::Stop);
}

#[test]
fn stop_resets_statistics() {
  let strategy = SupervisorStrategy::new(SupervisorStrategyKind::OneForOne, 3, Duration::from_secs(5), stop_only);
  let mut stats = RestartStatistics::new();
  let error = ActorError::recoverable("err");

  stats.record_failure(Duration::from_secs(1), Duration::from_secs(5), None);
  let outcome = strategy.handle_failure(&mut stats, &error, Duration::from_secs(3));
  assert_eq!(outcome, SupervisorDirective::Stop);
  assert_eq!(stats.failure_count(), 0);
}
