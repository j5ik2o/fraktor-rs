//! Strategy decision returned by split-brain resolver evaluation.

#[cfg(test)]
#[path = "downing_strategy_decision_test.rs"]
mod tests;

use alloc::vec::Vec;

use fraktor_remote_core_rs::address::UniqueAddress;

use super::{DowningDecision, DowningDecisionTrace};

/// Immutable decision result produced by a downing strategy.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DowningStrategyDecision {
  simple_decision:    DowningDecision,
  trace:              DowningDecisionTrace,
  retained_partition: Vec<UniqueAddress>,
  downing_targets:    Vec<UniqueAddress>,
  all_down:           bool,
}

impl DowningStrategyDecision {
  /// Creates a keep decision with retained and downing partitions.
  #[must_use]
  pub const fn keep(
    trace: DowningDecisionTrace,
    retained_partition: Vec<UniqueAddress>,
    downing_targets: Vec<UniqueAddress>,
  ) -> Self {
    Self { simple_decision: DowningDecision::Keep, trace, retained_partition, downing_targets, all_down: false }
  }

  /// Creates a down decision for explicit targets.
  #[must_use]
  pub const fn down(trace: DowningDecisionTrace, downing_targets: Vec<UniqueAddress>) -> Self {
    Self {
      simple_decision: DowningDecision::Down,
      trace,
      retained_partition: Vec::new(),
      downing_targets,
      all_down: false,
    }
  }

  /// Creates a defer decision with trace-only explanation.
  #[must_use]
  pub const fn defer(trace: DowningDecisionTrace) -> Self {
    Self {
      simple_decision: DowningDecision::Defer,
      trace,
      retained_partition: Vec::new(),
      downing_targets: Vec::new(),
      all_down: false,
    }
  }

  /// Creates an all-down decision for every observed target.
  #[must_use]
  pub const fn all_down(trace: DowningDecisionTrace, downing_targets: Vec<UniqueAddress>) -> Self {
    Self {
      simple_decision: DowningDecision::Down,
      trace,
      retained_partition: Vec::new(),
      downing_targets,
      all_down: true,
    }
  }

  /// Returns the simple provider-facing decision category.
  #[must_use]
  pub const fn simple_decision(&self) -> DowningDecision {
    self.simple_decision
  }

  /// Returns the trace explaining this decision.
  #[must_use]
  pub const fn trace(&self) -> &DowningDecisionTrace {
    &self.trace
  }

  /// Returns members retained by the strategy.
  #[must_use]
  pub const fn retained_partition(&self) -> &[UniqueAddress] {
    self.retained_partition.as_slice()
  }

  /// Returns members selected for downing.
  #[must_use]
  pub const fn downing_targets(&self) -> &[UniqueAddress] {
    self.downing_targets.as_slice()
  }

  /// Returns true when the strategy selected all observed members for downing.
  #[must_use]
  pub const fn is_all_down(&self) -> bool {
    self.all_down
  }
}
