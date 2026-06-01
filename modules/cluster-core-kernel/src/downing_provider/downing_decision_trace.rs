//! Trace details for split-brain resolver decisions.

use alloc::string::String;
use core::time::Duration;

use super::SplitBrainResolverStrategy;

/// Observable explanation attached to a strategy decision.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DowningDecisionTrace {
  strategy:              SplitBrainResolverStrategy,
  reason:                String,
  tie_break_rule:        Option<String>,
  stable_after_required: Option<Duration>,
  down_all_timeout:      Option<Duration>,
}

impl DowningDecisionTrace {
  /// Creates a trace for a deferred decision.
  #[must_use]
  pub const fn defer(strategy: SplitBrainResolverStrategy, reason: String) -> Self {
    Self { strategy, reason, tie_break_rule: None, stable_after_required: None, down_all_timeout: None }
  }

  /// Creates a trace for a selected majority partition.
  #[must_use]
  pub const fn majority_partition(strategy: SplitBrainResolverStrategy, reason: String) -> Self {
    Self { strategy, reason, tie_break_rule: None, stable_after_required: None, down_all_timeout: None }
  }

  /// Creates a trace for a stable-after defer.
  #[must_use]
  pub const fn stable_after_pending(
    strategy: SplitBrainResolverStrategy,
    stable_after_required: Duration,
    reason: String,
  ) -> Self {
    Self {
      strategy,
      reason,
      tie_break_rule: None,
      stable_after_required: Some(stable_after_required),
      down_all_timeout: None,
    }
  }

  /// Creates a trace for a pending all-down timeout decision.
  #[must_use]
  pub const fn down_all_pending(
    strategy: SplitBrainResolverStrategy,
    down_all_timeout: Duration,
    reason: String,
  ) -> Self {
    Self {
      strategy,
      reason,
      tie_break_rule: None,
      stable_after_required: None,
      down_all_timeout: Some(down_all_timeout),
    }
  }

  /// Creates a trace for an all-down timeout decision.
  #[must_use]
  pub const fn down_all_elapsed(
    strategy: SplitBrainResolverStrategy,
    down_all_timeout: Duration,
    reason: String,
  ) -> Self {
    Self {
      strategy,
      reason,
      tie_break_rule: None,
      stable_after_required: None,
      down_all_timeout: Some(down_all_timeout),
    }
  }

  /// Attaches the deterministic tie-break rule or defer reason.
  #[must_use]
  pub fn with_tie_break(mut self, tie_break_rule: String) -> Self {
    self.tie_break_rule = Some(tie_break_rule);
    self
  }

  /// Returns the strategy that produced the trace.
  #[must_use]
  pub const fn strategy(&self) -> SplitBrainResolverStrategy {
    self.strategy
  }

  /// Returns the primary decision reason.
  #[must_use]
  pub const fn reason(&self) -> &str {
    self.reason.as_str()
  }

  /// Returns the tie-break rule or tie defer reason.
  #[must_use]
  pub fn tie_break_rule(&self) -> Option<&str> {
    self.tie_break_rule.as_deref()
  }

  /// Returns the stable-after prerequisite when it blocked a decision.
  #[must_use]
  pub const fn stable_after_required(&self) -> Option<Duration> {
    self.stable_after_required
  }

  /// Returns the down-all timeout that allowed all members to be downed.
  #[must_use]
  pub const fn down_all_timeout(&self) -> Option<Duration> {
    self.down_all_timeout
  }
}
