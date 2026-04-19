use core::time::Duration;

use super::{
  super::{supervisor_directive::SupervisorDirective, supervisor_strategy_kind::SupervisorStrategyKind},
  SupervisorStrategy,
};
use crate::core::kernel::actor::{error::ActorError, supervision::RestartStatistics};

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

#[test]
fn with_decider_creates_strategy_using_dyn_decider() {
  let strategy = SupervisorStrategy::with_decider(|_| SupervisorDirective::Resume);
  let error = ActorError::recoverable("test");
  assert_eq!(strategy.decide(&error), SupervisorDirective::Resume);
}

#[test]
fn with_decider_dyn_decider_takes_priority_over_default() {
  let strategy = SupervisorStrategy::with_decider(|_| SupervisorDirective::Escalate);
  // default_decider would return Restart for recoverable, but dyn_decider overrides.
  let error = ActorError::recoverable("test");
  assert_eq!(strategy.decide(&error), SupervisorDirective::Escalate);
  // default_decider would return Stop for fatal, but dyn_decider overrides.
  let fatal = ActorError::fatal("fatal");
  assert_eq!(strategy.decide(&fatal), SupervisorDirective::Escalate);
}

#[test]
fn default_logging_enabled_is_true() {
  let strategy = SupervisorStrategy::default();
  assert!(strategy.logging_enabled());
}

#[test]
fn default_log_level_is_error() {
  use crate::core::kernel::event::logging::LogLevel;
  let strategy = SupervisorStrategy::default();
  assert_eq!(strategy.log_level(), LogLevel::Error);
}

#[test]
fn with_logging_enabled_sets_value() {
  let strategy = SupervisorStrategy::default().with_logging_enabled(false);
  assert!(!strategy.logging_enabled());
}

#[test]
fn with_log_level_sets_value() {
  use crate::core::kernel::event::logging::LogLevel;
  let strategy = SupervisorStrategy::default().with_log_level(LogLevel::Warn);
  assert_eq!(strategy.log_level(), LogLevel::Warn);
}

#[test]
fn default_decider_escalates_for_escalate_variant() {
  // SP-H1: `with_decider` が構築する strategy 経路全体で `Escalate` variant → Escalate directive が
  // 網羅的に扱われることを確認する。内部の `default_decider` は `dyn_decider` に shadow されるため、
  // ユーザクロージャを `ActorError` の全 variant を網羅する形で渡すことで、`Escalate` variant 追加と
  // decider 網羅性を強制する。
  let strategy = SupervisorStrategy::with_decider(|error| match error {
    | ActorError::Recoverable(_) => SupervisorDirective::Restart,
    | ActorError::Fatal(_) => SupervisorDirective::Stop,
    | ActorError::Escalate(_) => SupervisorDirective::Escalate,
  });
  assert_eq!(strategy.decide(&ActorError::escalate("boom")), SupervisorDirective::Escalate);
}

#[test]
fn default_strategy_escalates_for_escalate_variant() {
  // SP-H1: `Default for SupervisorStrategy` の内部 `decider`（fn ポインタ）で
  // `Escalate` variant → Escalate directive にマップされることを確認する。
  // `decide()` は `dyn_decider` が `None` の場合に fn ポインタを直接呼ぶため、
  // `default()` 経由で観測可能。
  let strategy = SupervisorStrategy::default();
  assert_eq!(strategy.decide(&ActorError::escalate("boom")), SupervisorDirective::Escalate);
}

#[test]
fn default_decider_preserves_existing_mappings() {
  // SP-H1: `Escalate` variant 追加で既存の `Recoverable → Restart` / `Fatal → Stop`
  // マッピングが回帰しないことを明示的にアサートする（回帰防止）。
  let strategy = SupervisorStrategy::default();
  assert_eq!(
    strategy.decide(&ActorError::recoverable("recoverable")),
    SupervisorDirective::Restart
  );
  assert_eq!(strategy.decide(&ActorError::fatal("fatal")), SupervisorDirective::Stop);
}
