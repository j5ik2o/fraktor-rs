//! Supervisor configuration and decision logic.

use core::{
  fmt::{Debug, Formatter, Result as FmtResult},
  time::Duration,
};

use fraktor_utils_core_rs::core::sync::ArcShared;

use super::{
  restart_limit::RestartLimit, supervisor_directive::SupervisorDirective,
  supervisor_strategy_kind::SupervisorStrategyKind,
};
use crate::core::kernel::{
  actor::{error::ActorError, supervision::restart_statistics::RestartStatistics},
  event::logging::LogLevel,
};

#[cfg(test)]
mod tests;

type SupervisorDecider = fn(&ActorError) -> SupervisorDirective;

/// Boxed closure decider supporting type-discriminated supervision chains.
type DynDecider = ArcShared<dyn Fn(&ActorError) -> SupervisorDirective + Send + Sync>;

const DEFAULT_STASH_CAPACITY: usize = 1000;

/// Supervisor configuration controlling restart policies.
///
/// `within: Duration::ZERO` is the fraktor-rs sentinel for "no window"
/// (equivalent to typed Pekko `withinTimeRange = Duration.Zero` and to
/// classic Pekko `withinTimeRangeOption` returning `None`).
#[derive(Clone)]
pub struct SupervisorStrategy {
  kind:            SupervisorStrategyKind,
  max_restarts:    RestartLimit,
  within:          Duration,
  decider:         SupervisorDecider,
  dyn_decider:     Option<DynDecider>,
  stop_children:   bool,
  stash_capacity:  usize,
  logging_enabled: bool,
  log_level:       LogLevel,
}

impl Debug for SupervisorStrategy {
  fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
    f.debug_struct("SupervisorStrategy")
      .field("kind", &self.kind)
      .field("max_restarts", &self.max_restarts)
      .field("within", &self.within)
      .field("has_dyn_decider", &self.dyn_decider.is_some())
      .field("stop_children", &self.stop_children)
      .field("stash_capacity", &self.stash_capacity)
      .field("logging_enabled", &self.logging_enabled)
      .field("log_level", &self.log_level)
      .finish()
  }
}

impl SupervisorStrategy {
  /// Creates a supervisor strategy with a function pointer decider.
  ///
  /// `max_restarts` uses the [`RestartLimit`] contract (Pekko
  /// `maxNrOfRetries`): `Unlimited` for unbounded retries, `WithinWindow(0)`
  /// for immediate stop, `WithinWindow(n)` for up to `n` restarts within
  /// the `within` window. `within = Duration::ZERO` disables the window.
  #[must_use]
  pub const fn new(
    kind: SupervisorStrategyKind,
    max_restarts: RestartLimit,
    within: Duration,
    decider: SupervisorDecider,
  ) -> Self {
    Self {
      kind,
      max_restarts,
      within,
      decider,
      dyn_decider: None,
      stop_children: true,
      stash_capacity: DEFAULT_STASH_CAPACITY,
      logging_enabled: true,
      log_level: LogLevel::Error,
    }
  }

  /// Creates a supervisor strategy with a closure-based decider.
  ///
  /// This allows type-discriminated supervision chains that capture state.
  #[must_use]
  pub fn with_decider<F>(decider: F) -> Self
  where
    F: Fn(&ActorError) -> SupervisorDirective + Send + Sync + 'static, {
    const fn default_decider(error: &ActorError) -> SupervisorDirective {
      match error {
        | ActorError::Recoverable(_) => SupervisorDirective::Restart,
        | ActorError::Fatal(_) => SupervisorDirective::Stop,
        | ActorError::Escalate(_) => SupervisorDirective::Escalate,
      }
    }
    Self {
      kind:            SupervisorStrategyKind::OneForOne,
      max_restarts:    RestartLimit::WithinWindow(10),
      within:          Duration::from_secs(1),
      decider:         default_decider,
      dyn_decider:     Some(ArcShared::new(decider)),
      stop_children:   true,
      stash_capacity:  DEFAULT_STASH_CAPACITY,
      logging_enabled: true,
      log_level:       LogLevel::Error,
    }
  }

  /// Replaces the effective decider while preserving the rest of the strategy
  /// configuration.
  #[must_use]
  pub fn with_dyn_decider<F>(mut self, decider: F) -> Self
  where
    F: Fn(&ActorError) -> SupervisorDirective + Send + Sync + 'static, {
    self.dyn_decider = Some(ArcShared::new(decider));
    self
  }

  /// Evaluates the supervisor directive for the provided error.
  #[must_use]
  pub fn decide(&self, error: &ActorError) -> SupervisorDirective {
    if let Some(ref dyn_decider) = self.dyn_decider {
      return dyn_decider(error);
    }
    (self.decider)(error)
  }

  /// Applies restart accounting and returns the effective directive.
  ///
  /// Directive handling mirrors Pekko `FaultHandling.scala` exactly:
  /// - `Restart` delegates to [`RestartStatistics::request_restart_permission`]; if it returns
  ///   `false` the directive is promoted to [`SupervisorDirective::Stop`] and statistics are reset
  ///   (mirroring the effect of Pekko's `processFailure(false, ...)` stopping the child and tearing
  ///   down its stats).
  /// - `Stop` / `Escalate` reset statistics.
  /// - `Resume` leaves statistics untouched — Pekko's `Resume` branch does not touch `childStats`.
  ///
  /// `now` must be a monotonic clock reading (e.g.
  /// `ActorSystem::monotonic_now()`).
  #[must_use]
  pub fn handle_failure(
    &self,
    statistics: &mut RestartStatistics,
    error: &ActorError,
    now: Duration,
  ) -> SupervisorDirective {
    match self.decide(error) {
      | SupervisorDirective::Restart => {
        if statistics.request_restart_permission(now, self.max_restarts, self.within) {
          SupervisorDirective::Restart
        } else {
          statistics.reset();
          SupervisorDirective::Stop
        }
      },
      | SupervisorDirective::Stop => {
        statistics.reset();
        SupervisorDirective::Stop
      },
      | SupervisorDirective::Escalate => {
        statistics.reset();
        SupervisorDirective::Escalate
      },
      | SupervisorDirective::Resume => SupervisorDirective::Resume,
    }
  }

  /// Returns the strategy kind.
  #[must_use]
  pub const fn kind(&self) -> SupervisorStrategyKind {
    self.kind
  }

  /// Returns the configured restart limit policy.
  #[must_use]
  pub const fn max_restarts(&self) -> RestartLimit {
    self.max_restarts
  }

  /// Returns the time window used when counting restarts.
  #[must_use]
  pub const fn within(&self) -> Duration {
    self.within
  }

  /// Returns whether sibling children are stopped on restart.
  #[must_use]
  pub const fn stop_children(&self) -> bool {
    self.stop_children
  }

  /// Returns the stash capacity.
  #[must_use]
  pub const fn stash_capacity(&self) -> usize {
    self.stash_capacity
  }

  /// Sets whether sibling children should be stopped on restart.
  #[must_use]
  pub const fn with_stop_children(mut self, stop_children: bool) -> Self {
    self.stop_children = stop_children;
    self
  }

  /// Sets the stash capacity for message buffering during restart.
  #[must_use]
  pub const fn with_stash_capacity(mut self, stash_capacity: usize) -> Self {
    self.stash_capacity = stash_capacity;
    self
  }

  /// Returns whether failure logging is enabled.
  #[must_use]
  pub const fn logging_enabled(&self) -> bool {
    self.logging_enabled
  }

  /// Returns the log level used for failure events.
  #[must_use]
  pub const fn log_level(&self) -> LogLevel {
    self.log_level
  }

  /// Sets whether failure logging is enabled.
  #[must_use]
  pub const fn with_logging_enabled(mut self, enabled: bool) -> Self {
    self.logging_enabled = enabled;
    self
  }

  /// Sets the log level for failure events.
  #[must_use]
  pub const fn with_log_level(mut self, level: LogLevel) -> Self {
    self.log_level = level;
    self
  }

  /// Sets the strategy kind.
  #[must_use]
  pub const fn with_kind(mut self, kind: SupervisorStrategyKind) -> Self {
    self.kind = kind;
    self
  }
}

impl Default for SupervisorStrategy {
  fn default() -> Self {
    const fn decider(error: &ActorError) -> SupervisorDirective {
      match error {
        | ActorError::Recoverable(_) => SupervisorDirective::Restart,
        | ActorError::Fatal(_) => SupervisorDirective::Stop,
        | ActorError::Escalate(_) => SupervisorDirective::Escalate,
      }
    }

    Self::new(SupervisorStrategyKind::OneForOne, RestartLimit::WithinWindow(10), Duration::from_secs(1), decider)
  }
}
