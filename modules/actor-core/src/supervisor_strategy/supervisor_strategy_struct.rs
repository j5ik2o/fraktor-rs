use core::time::Duration;

use super::{supervisor_directive::SupervisorDirective, supervisor_strategy_kind::SupervisorStrategyKind};
use crate::{actor_error::ActorError, restart_statistics::RestartStatistics};

type SupervisorDecider = fn(&ActorError) -> SupervisorDirective;

/// Supervisor configuration controlling restart policies.
#[derive(Clone, Copy, Debug)]
pub struct SupervisorStrategy {
  kind:         SupervisorStrategyKind,
  max_restarts: u32,
  within:       Duration,
  decider:      SupervisorDecider,
}

impl SupervisorStrategy {
  /// Creates a supervisor strategy.
  #[must_use]
  pub const fn new(
    kind: SupervisorStrategyKind,
    max_restarts: u32,
    within: Duration,
    decider: SupervisorDecider,
  ) -> Self {
    Self { kind, max_restarts, within, decider }
  }

  /// Evaluates the supervisor directive for the provided error.
  #[must_use]
  pub fn decide(&self, error: &ActorError) -> SupervisorDirective {
    (self.decider)(error)
  }

  /// Handles a failure by applying the restart policy and updating statistics.
  ///
  /// When the decider returns [`SupervisorDirective::Restart`], the restart counter is incremented
  /// and compared against the configured limit. Exceeding the limit results in a
  /// [`SupervisorDirective::Stop`] outcome. Any other directive resets the statistics.
  #[must_use]
  pub fn handle_failure(
    &self,
    statistics: &mut RestartStatistics,
    error: &ActorError,
    now: Duration,
  ) -> SupervisorDirective {
    match self.decide(error) {
      | SupervisorDirective::Restart => {
        let limit = if self.max_restarts == 0 { None } else { Some(self.max_restarts) };
        let count = statistics.record_failure(now, self.within, limit);
        if self.max_restarts > 0 && count as u32 > self.max_restarts {
          statistics.reset();
          SupervisorDirective::Stop
        } else {
          SupervisorDirective::Restart
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
    }
  }

  /// Returns the strategy kind.
  #[must_use]
  pub const fn kind(&self) -> SupervisorStrategyKind {
    self.kind
  }

  /// Returns the restart limit.
  #[must_use]
  pub const fn max_restarts(&self) -> u32 {
    self.max_restarts
  }

  /// Returns the time window for restart counting.
  #[must_use]
  pub const fn within(&self) -> Duration {
    self.within
  }
}

#[cfg(test)]
mod tests {
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
}
