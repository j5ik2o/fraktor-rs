use core::time::Duration;

use super::{
  super::{
    restart_limit::RestartLimit, supervisor_directive::SupervisorDirective,
    supervisor_strategy_kind::SupervisorStrategyKind,
  },
  SupervisorStrategy,
};
use crate::actor::{error::ActorError, supervision::RestartStatistics};

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
  let strategy = SupervisorStrategy::new(
    SupervisorStrategyKind::OneForOne,
    RestartLimit::WithinWindow(3),
    Duration::from_secs(5),
    restart_only,
  );
  let outcome = strategy.handle_failure(&mut stats, &ActorError::recoverable("fail"), Duration::from_secs(1));
  assert_eq!(outcome, SupervisorDirective::Restart);
  assert_eq!(stats.restart_count(), 1);
}

#[test]
fn exceeding_limit_forces_stop() {
  let mut stats = RestartStatistics::new();
  let strategy = SupervisorStrategy::new(
    SupervisorStrategyKind::OneForOne,
    RestartLimit::WithinWindow(1),
    Duration::from_secs(5),
    restart_only,
  );
  let first = strategy.handle_failure(&mut stats, &ActorError::recoverable("fail"), Duration::from_secs(1));
  let second = strategy.handle_failure(&mut stats, &ActorError::recoverable("fail"), Duration::from_secs(2));
  assert_eq!(first, SupervisorDirective::Restart);
  assert_eq!(second, SupervisorDirective::Stop);
  assert_eq!(stats.restart_count(), 0);
}

#[test]
fn non_restart_resets_statistics() {
  let mut stats = RestartStatistics::new();
  let strategy = SupervisorStrategy::new(
    SupervisorStrategyKind::OneForOne,
    RestartLimit::WithinWindow(3),
    Duration::from_secs(5),
    stop_only,
  );
  // Prime statistics via the permission path so the reset can be observed.
  stats.request_restart_permission(Duration::from_secs(1), RestartLimit::WithinWindow(3), Duration::from_secs(5));
  assert!(stats.restart_count() > 0);
  let decision = strategy.handle_failure(&mut stats, &ActorError::recoverable("fail"), Duration::from_secs(2));
  assert_eq!(decision, SupervisorDirective::Stop);
  assert_eq!(stats.restart_count(), 0);
  assert_eq!(stats.window_start(), None);
}

#[test]
fn resume_leaves_statistics_unchanged() {
  let mut stats = RestartStatistics::new();
  let strategy = SupervisorStrategy::new(
    SupervisorStrategyKind::OneForOne,
    RestartLimit::WithinWindow(3),
    Duration::from_secs(5),
    resume_only,
  );
  stats.request_restart_permission(Duration::from_secs(1), RestartLimit::WithinWindow(3), Duration::from_secs(5));
  let count_before = stats.restart_count();
  let window_before = stats.window_start();
  let decision = strategy.handle_failure(&mut stats, &ActorError::recoverable("fail"), Duration::from_secs(2));
  assert_eq!(decision, SupervisorDirective::Resume);
  // Pekko parity: Resume must not touch restart stats.
  assert_eq!(stats.restart_count(), count_before);
  assert_eq!(stats.window_start(), window_before);
}

#[test]
fn within_window_zero_stops_immediately_without_counter_update() {
  // Pekko `(Some(0), _) if retries < 1 => false`. Counter must not be updated.
  let mut stats = RestartStatistics::new();
  let strategy = SupervisorStrategy::new(
    SupervisorStrategyKind::OneForOne,
    RestartLimit::WithinWindow(0),
    Duration::from_secs(5),
    restart_only,
  );
  let outcome = strategy.handle_failure(&mut stats, &ActorError::recoverable("fail"), Duration::from_secs(1));
  assert_eq!(outcome, SupervisorDirective::Stop);
  assert_eq!(stats.restart_count(), 0);
  assert_eq!(stats.window_start(), None);
}

#[test]
fn unlimited_no_window_never_updates_counter() {
  // Pekko `(None, _) => true`.
  let mut stats = RestartStatistics::new();
  let strategy =
    SupervisorStrategy::new(SupervisorStrategyKind::OneForOne, RestartLimit::Unlimited, Duration::ZERO, restart_only);
  for i in 0..50 {
    let out = strategy.handle_failure(&mut stats, &ActorError::recoverable("fail"), Duration::from_millis(i));
    assert_eq!(out, SupervisorDirective::Restart);
  }
  assert_eq!(stats.restart_count(), 0);
  assert_eq!(stats.window_start(), None);
}

#[test]
fn unlimited_with_window_denies_second_in_window_failure() {
  // Pekko `(None, Some(window)) => retriesInWindowOkay(1, window)`: retries = 1
  // fixed, so the second in-window failure is denied.
  let mut stats = RestartStatistics::new();
  let strategy = SupervisorStrategy::new(
    SupervisorStrategyKind::OneForOne,
    RestartLimit::Unlimited,
    Duration::from_secs(10),
    restart_only,
  );
  let first = strategy.handle_failure(&mut stats, &ActorError::recoverable("fail"), Duration::from_secs(0));
  assert_eq!(first, SupervisorDirective::Restart);
  assert_eq!(stats.restart_count(), 1);
  assert_eq!(stats.window_start(), Some(Duration::from_secs(0)));
  let second = strategy.handle_failure(&mut stats, &ActorError::recoverable("fail"), Duration::from_secs(5));
  assert_eq!(second, SupervisorDirective::Stop);
  assert_eq!(stats.restart_count(), 0);
  assert_eq!(stats.window_start(), None);
}

#[test]
fn within_window_n_resets_counter_on_window_expiry() {
  // Pekko `retriesInWindowOkay` outside-window branch: count = 1, window_start = now, true.
  let mut stats = RestartStatistics::new();
  let strategy = SupervisorStrategy::new(
    SupervisorStrategyKind::OneForOne,
    RestartLimit::WithinWindow(3),
    Duration::from_secs(10),
    restart_only,
  );
  // Prime: 2 failures inside the window starting at 0s.
  let first = strategy.handle_failure(&mut stats, &ActorError::recoverable("fail"), Duration::from_secs(0));
  let second = strategy.handle_failure(&mut stats, &ActorError::recoverable("fail"), Duration::from_secs(5));
  assert_eq!(first, SupervisorDirective::Restart);
  assert_eq!(second, SupervisorDirective::Restart);
  assert_eq!(stats.restart_count(), 2);
  assert_eq!(stats.window_start(), Some(Duration::from_secs(0)));
  // 15s is outside the 10s window ⇒ counter resets to 1, window_start to now, restart.
  let outcome = strategy.handle_failure(&mut stats, &ActorError::recoverable("fail"), Duration::from_secs(15));
  assert_eq!(outcome, SupervisorDirective::Restart);
  assert_eq!(stats.restart_count(), 1);
  assert_eq!(stats.window_start(), Some(Duration::from_secs(15)));
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
  use crate::event::logging::LogLevel;
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
  use crate::event::logging::LogLevel;
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
  assert_eq!(strategy.decide(&ActorError::recoverable("recoverable")), SupervisorDirective::Restart);
  assert_eq!(strategy.decide(&ActorError::fatal("fatal")), SupervisorDirective::Stop);
}
