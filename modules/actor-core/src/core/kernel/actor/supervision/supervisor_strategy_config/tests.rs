use core::time::Duration;

use super::{
  super::{
    backoff_supervisor_strategy::BackoffSupervisorStrategy, base::SupervisorStrategy, restart_limit::RestartLimit,
    supervisor_directive::SupervisorDirective, supervisor_strategy_kind::SupervisorStrategyKind,
  },
  SupervisorStrategyConfig,
};
use crate::core::kernel::{
  actor::{error::ActorError, supervision::RestartStatistics},
  event::logging::LogLevel,
};

#[test]
fn standard_decide_delegates_to_inner() {
  let strategy = SupervisorStrategy::new(
    SupervisorStrategyKind::OneForOne,
    RestartLimit::WithinWindow(3),
    Duration::from_secs(5),
    |_| SupervisorDirective::Resume,
  );
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
  let strategy = SupervisorStrategy::new(
    SupervisorStrategyKind::OneForOne,
    RestartLimit::WithinWindow(3),
    Duration::from_secs(5),
    |_| SupervisorDirective::Restart,
  );
  let config = SupervisorStrategyConfig::Standard(strategy);
  let mut stats = RestartStatistics::new();
  let directive = config.handle_failure(&mut stats, &ActorError::recoverable("fail"), Duration::from_secs(1));
  assert_eq!(directive, SupervisorDirective::Restart);
  assert_eq!(stats.restart_count(), 1);
}

#[test]
fn backoff_handle_failure_restarts_within_limit() {
  let backoff = BackoffSupervisorStrategy::new(Duration::from_millis(100), Duration::from_secs(10), 0.0)
    .with_max_restarts(RestartLimit::WithinWindow(3));
  let config = SupervisorStrategyConfig::Backoff(backoff);
  let mut stats = RestartStatistics::new();
  let directive = config.handle_failure(&mut stats, &ActorError::recoverable("fail"), Duration::from_secs(1));
  assert_eq!(directive, SupervisorDirective::Restart);
}

#[test]
fn backoff_handle_failure_stops_when_exceeding_limit() {
  let backoff = BackoffSupervisorStrategy::new(Duration::from_millis(100), Duration::from_secs(10), 0.0)
    .with_max_restarts(RestartLimit::WithinWindow(1));
  let config = SupervisorStrategyConfig::Backoff(backoff);
  let mut stats = RestartStatistics::new();
  let first = config.handle_failure(&mut stats, &ActorError::recoverable("fail"), Duration::from_secs(1));
  let second = config.handle_failure(&mut stats, &ActorError::recoverable("fail"), Duration::from_secs(2));
  assert_eq!(first, SupervisorDirective::Restart);
  assert_eq!(second, SupervisorDirective::Stop);
}

#[test]
fn backoff_handle_failure_unlimited_restarts() {
  // Pekko parity: `Unlimited + reset_backoff_after` invokes
  // `retriesInWindowOkay(retries = 1, window)` — with finite window only
  // the very first failure is permitted. This test uses a zero-window
  // reset configuration by exercising the `Unlimited + window = ZERO` arm
  // (`(None, _) => true`).
  let backoff = BackoffSupervisorStrategy::new(Duration::from_millis(100), Duration::from_secs(10), 0.0)
    .with_max_restarts(RestartLimit::Unlimited)
    .with_reset_backoff_after(Duration::ZERO);
  let config = SupervisorStrategyConfig::Backoff(backoff);
  let mut stats = RestartStatistics::new();
  for i in 0..10 {
    let directive = config.handle_failure(&mut stats, &ActorError::recoverable("fail"), Duration::from_secs(i as u64));
    assert_eq!(directive, SupervisorDirective::Restart);
  }
}

#[test]
fn standard_kind_returns_inner_kind() {
  let strategy = SupervisorStrategy::new(
    SupervisorStrategyKind::AllForOne,
    RestartLimit::WithinWindow(3),
    Duration::from_secs(5),
    |_| SupervisorDirective::Restart,
  );
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
  // リセットを観測できるように、先に in-window 失敗履歴を記録しておく。
  stats.request_restart_permission(Duration::from_secs(1), RestartLimit::WithinWindow(3), Duration::from_secs(10));
  assert_eq!(stats.restart_count(), 1);

  let _directive = config.handle_failure(&mut stats, &ActorError::fatal("fatal"), Duration::from_secs(2));
  assert_eq!(stats.restart_count(), 0);
  assert_eq!(stats.window_start(), None);
}

#[test]
fn standard_logging_enabled_delegates_to_inner() {
  let standard = SupervisorStrategyConfig::Standard(SupervisorStrategy::default().with_logging_enabled(false));
  assert!(!standard.logging_enabled());

  let standard_enabled = SupervisorStrategyConfig::Standard(SupervisorStrategy::default());
  assert!(standard_enabled.logging_enabled());
}

#[test]
fn backoff_logging_enabled_delegates_to_inner() {
  let backoff = SupervisorStrategyConfig::Backoff(
    BackoffSupervisorStrategy::new(Duration::from_millis(100), Duration::from_secs(10), 0.0)
      .with_logging_enabled(false),
  );
  assert!(!backoff.logging_enabled());

  let backoff_enabled = SupervisorStrategyConfig::Backoff(BackoffSupervisorStrategy::new(
    Duration::from_millis(100),
    Duration::from_secs(10),
    0.0,
  ));
  assert!(backoff_enabled.logging_enabled());
}

#[test]
fn standard_effective_log_level_returns_configured_level() {
  let standard = SupervisorStrategyConfig::Standard(SupervisorStrategy::default().with_log_level(LogLevel::Warn));
  // Standard always returns its log_level regardless of error_count.
  assert_eq!(standard.effective_log_level(0), LogLevel::Warn);
  assert_eq!(standard.effective_log_level(100), LogLevel::Warn);
}

#[test]
fn backoff_effective_log_level_delegates_threshold_logic() {
  let backoff = SupervisorStrategyConfig::Backoff(
    BackoffSupervisorStrategy::new(Duration::from_millis(100), Duration::from_secs(10), 0.0)
      .with_log_level(LogLevel::Error)
      .with_critical_log_level(LogLevel::Warn, 3),
  );
  assert_eq!(backoff.effective_log_level(0), LogLevel::Error);
  assert_eq!(backoff.effective_log_level(2), LogLevel::Error);
  assert_eq!(backoff.effective_log_level(3), LogLevel::Warn);
  assert_eq!(backoff.effective_log_level(10), LogLevel::Warn);
}

#[test]
fn backoff_decide_escalates_for_escalate_variant() {
  // SP-H1: `backoff_decide` で `Escalate` variant → Escalate directive にマップされることを確認する。
  // `backoff_decide` は private `const fn`
  // のため、`SupervisorStrategyConfig::Backoff(...).decide(..)` 経由で呼び出して観測する。
  let backoff = BackoffSupervisorStrategy::new(Duration::from_millis(100), Duration::from_secs(10), 0.0);
  let config = SupervisorStrategyConfig::Backoff(backoff);
  assert_eq!(config.decide(&ActorError::escalate("boom")), SupervisorDirective::Escalate);
}
