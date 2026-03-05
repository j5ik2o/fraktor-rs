use core::time::Duration;

use super::{
  super::{supervisor_directive::SupervisorDirective, supervisor_strategy_kind::SupervisorStrategyKind},
  SupervisorStrategy,
};
use crate::core::{error::ActorError, supervision::RestartStatistics};

fn restart_only(_error: &ActorError) -> SupervisorDirective {
  SupervisorDirective::Restart
}

fn stop_only(_error: &ActorError) -> SupervisorDirective {
  SupervisorDirective::Stop
}

fn resume_only(_error: &ActorError) -> SupervisorDirective {
  SupervisorDirective::Resume
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

#[test]
fn resume_leaves_statistics_unchanged() {
  let mut stats = RestartStatistics::new();
  let strategy = SupervisorStrategy::new(SupervisorStrategyKind::OneForOne, 3, Duration::from_secs(5), resume_only);
  stats.record_failure(Duration::from_secs(1), Duration::from_secs(5), Some(3));
  let count_before = stats.failure_count();
  let decision = strategy.handle_failure(&mut stats, &ActorError::recoverable("fail"), Duration::from_secs(2));
  assert_eq!(decision, SupervisorDirective::Resume);
  assert_eq!(stats.failure_count(), count_before);
}

#[test]
fn default_stop_children_is_true() {
  let strategy = SupervisorStrategy::default();
  assert!(strategy.stop_children());
}

#[test]
fn default_stash_capacity_is_1000() {
  let strategy = SupervisorStrategy::default();
  assert_eq!(strategy.stash_capacity(), 1000);
}

#[test]
fn with_stop_children_sets_value() {
  let strategy = SupervisorStrategy::default().with_stop_children(false);
  assert!(!strategy.stop_children());
}

#[test]
fn with_stash_capacity_sets_value() {
  let strategy = SupervisorStrategy::default().with_stash_capacity(500);
  assert_eq!(strategy.stash_capacity(), 500);
}
