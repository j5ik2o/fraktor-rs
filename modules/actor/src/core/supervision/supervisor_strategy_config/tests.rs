use core::time::Duration;

use super::{
  super::{
    backoff_supervisor_strategy::BackoffSupervisorStrategy, base::SupervisorStrategy,
    supervisor_directive::SupervisorDirective, supervisor_strategy_kind::SupervisorStrategyKind,
  },
  SupervisorStrategyConfig,
};
use crate::core::{error::ActorError, supervision::RestartStatistics};

#[test]
fn standard_decide_delegates_to_inner() {
  let strategy = SupervisorStrategy::new(SupervisorStrategyKind::OneForOne, 3, Duration::from_secs(5), |_| {
    SupervisorDirective::Resume
  });
  let config = SupervisorStrategyConfig::Standard(strategy);
  assert_eq!(config.decide(&ActorError::recoverable("test")), SupervisorDirective::Resume);
}

#[test]
fn backoff_decide_restarts_on_recoverable() {
  let backoff = BackoffSupervisorStrategy::new(Duration::from_millis(100), Duration::from_secs(10), 0.0);
  let config = SupervisorStrategyConfig::Backoff(backoff);
  assert_eq!(config.decide(&ActorError::recoverable("test")), SupervisorDirective::Restart);
}

#[test]
fn backoff_decide_stops_on_fatal() {
  let backoff = BackoffSupervisorStrategy::new(Duration::from_millis(100), Duration::from_secs(10), 0.0);
  let config = SupervisorStrategyConfig::Backoff(backoff);
  assert_eq!(config.decide(&ActorError::fatal("fatal")), SupervisorDirective::Stop);
}

#[test]
fn standard_handle_failure_delegates_to_inner() {
  let strategy = SupervisorStrategy::new(SupervisorStrategyKind::OneForOne, 3, Duration::from_secs(5), |_| {
    SupervisorDirective::Restart
  });
  let config = SupervisorStrategyConfig::Standard(strategy);
  let mut stats = RestartStatistics::new();
  let directive = config.handle_failure(&mut stats, &ActorError::recoverable("fail"), Duration::from_secs(1));
  assert_eq!(directive, SupervisorDirective::Restart);
  assert_eq!(stats.failure_count(), 1);
}

#[test]
fn backoff_handle_failure_restarts_within_limit() {
  let backoff =
    BackoffSupervisorStrategy::new(Duration::from_millis(100), Duration::from_secs(10), 0.0).with_max_restarts(3);
  let config = SupervisorStrategyConfig::Backoff(backoff);
  let mut stats = RestartStatistics::new();
  let directive = config.handle_failure(&mut stats, &ActorError::recoverable("fail"), Duration::from_secs(1));
  assert_eq!(directive, SupervisorDirective::Restart);
}

#[test]
fn backoff_handle_failure_stops_when_exceeding_limit() {
  let backoff =
    BackoffSupervisorStrategy::new(Duration::from_millis(100), Duration::from_secs(10), 0.0).with_max_restarts(1);
  let config = SupervisorStrategyConfig::Backoff(backoff);
  let mut stats = RestartStatistics::new();
  let first = config.handle_failure(&mut stats, &ActorError::recoverable("fail"), Duration::from_secs(1));
  let second = config.handle_failure(&mut stats, &ActorError::recoverable("fail"), Duration::from_secs(2));
  assert_eq!(first, SupervisorDirective::Restart);
  assert_eq!(second, SupervisorDirective::Stop);
}

#[test]
fn backoff_handle_failure_unlimited_restarts() {
  let backoff =
    BackoffSupervisorStrategy::new(Duration::from_millis(100), Duration::from_secs(10), 0.0).with_max_restarts(0);
  let config = SupervisorStrategyConfig::Backoff(backoff);
  let mut stats = RestartStatistics::new();
  for i in 0..10 {
    let directive = config.handle_failure(&mut stats, &ActorError::recoverable("fail"), Duration::from_secs(i as u64));
    assert_eq!(directive, SupervisorDirective::Restart);
  }
}

#[test]
fn standard_kind_returns_inner_kind() {
  let strategy = SupervisorStrategy::new(SupervisorStrategyKind::AllForOne, 3, Duration::from_secs(5), |_| {
    SupervisorDirective::Restart
  });
  let config = SupervisorStrategyConfig::Standard(strategy);
  assert_eq!(config.kind(), SupervisorStrategyKind::AllForOne);
}

#[test]
fn backoff_kind_is_one_for_one() {
  let backoff = BackoffSupervisorStrategy::new(Duration::from_millis(100), Duration::from_secs(10), 0.0);
  let config = SupervisorStrategyConfig::Backoff(backoff);
  assert_eq!(config.kind(), SupervisorStrategyKind::OneForOne);
}

#[test]
fn from_supervisor_strategy_creates_standard() {
  let strategy = SupervisorStrategy::default();
  let config: SupervisorStrategyConfig = strategy.into();
  assert!(matches!(config, SupervisorStrategyConfig::Standard(_)));
}

#[test]
fn from_backoff_creates_backoff() {
  let backoff = BackoffSupervisorStrategy::new(Duration::from_millis(100), Duration::from_secs(10), 0.0);
  let config: SupervisorStrategyConfig = backoff.into();
  assert!(matches!(config, SupervisorStrategyConfig::Backoff(_)));
}

#[test]
fn default_creates_standard() {
  let config = SupervisorStrategyConfig::default();
  assert!(matches!(config, SupervisorStrategyConfig::Standard(_)));
}

#[test]
fn stop_children_delegates_to_inner() {
  let standard = SupervisorStrategyConfig::Standard(SupervisorStrategy::default().with_stop_children(false));
  assert!(!standard.stop_children());

  let backoff = SupervisorStrategyConfig::Backoff(
    BackoffSupervisorStrategy::new(Duration::from_millis(100), Duration::from_secs(10), 0.0).with_stop_children(false),
  );
  assert!(!backoff.stop_children());
}

#[test]
fn stash_capacity_delegates_to_inner() {
  let standard = SupervisorStrategyConfig::Standard(SupervisorStrategy::default().with_stash_capacity(42));
  assert_eq!(standard.stash_capacity(), 42);

  let backoff = SupervisorStrategyConfig::Backoff(
    BackoffSupervisorStrategy::new(Duration::from_millis(100), Duration::from_secs(10), 0.0).with_stash_capacity(42),
  );
  assert_eq!(backoff.stash_capacity(), 42);
}

#[test]
fn backoff_handle_failure_stops_on_fatal() {
  let backoff = BackoffSupervisorStrategy::new(Duration::from_millis(100), Duration::from_secs(10), 0.0);
  let config = SupervisorStrategyConfig::Backoff(backoff);
  let mut stats = RestartStatistics::new();

  let directive = config.handle_failure(&mut stats, &ActorError::fatal("fatal"), Duration::from_secs(2));
  assert_eq!(directive, SupervisorDirective::Stop);
}

#[test]
fn backoff_handle_failure_resets_stats_on_fatal() {
  let backoff = BackoffSupervisorStrategy::new(Duration::from_millis(100), Duration::from_secs(10), 0.0);
  let config = SupervisorStrategyConfig::Backoff(backoff);
  let mut stats = RestartStatistics::new();
  // リセットを観測できるように、先に失敗履歴を記録しておく。
  stats.record_failure(Duration::from_secs(1), Duration::from_secs(10), None);
  assert_eq!(stats.failure_count(), 1);

  let _directive = config.handle_failure(&mut stats, &ActorError::fatal("fatal"), Duration::from_secs(2));
  assert_eq!(stats.failure_count(), 0);
}
