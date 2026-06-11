//! Split Brain Resolver evaluator.

#[cfg(test)]
#[path = "split_brain_resolver_test.rs"]
mod tests;

use alloc::{string::String, vec::Vec};
use core::time::Duration;

use fraktor_remote_core_rs::address::UniqueAddress;

use super::{
  DowningDecisionContext, DowningDecisionTrace, DowningStrategyDecision, LeaseAcquisitionOutcome, LeaseMajorityPort,
  SplitBrainResolverSettings, SplitBrainResolverStrategy,
};
use crate::membership::{NodeRecord, ReachabilityStatus, oldest_member};

const MEMBERSHIP_REQUIRED: &str = "membership snapshot is required for split-brain evaluation";
const NO_ACTIVE_MEMBERS: &str = "no active members are available for split-brain evaluation";
const REACHABLE_MAJORITY_SELECTED: &str = "reachable majority partition selected";
const NON_REACHABLE_MAJORITY_SELECTED: &str = "non-reachable majority partition selected";
const STATIC_QUORUM_SELECTED: &str = "reachable static quorum partition selected";
const NON_REACHABLE_STATIC_QUORUM_SELECTED: &str = "non-reachable static quorum partition selected";
const OLDEST_PARTITION_SELECTED: &str = "oldest member partition selected";
const MAJORITY_TIE: &str = "reachable and non-reachable partitions have equal size";
const STATIC_QUORUM_TIE: &str = "reachable and non-reachable partitions both satisfy static quorum";
const LOCAL_OBSERVER_DOWNING_TARGET: &str = "local observer is selected for downing";
const STATIC_QUORUM_SIZE_MISSING: &str = "static quorum size is not configured";
const STATIC_QUORUM_SIZE_ZERO: &str = "static quorum size must be greater than zero";
const EXPLICIT_DOWN_SELECTED: &str = "explicit down command selected";
const STABLE_AFTER_PENDING: &str = "membership has not been stable for the required duration";
const DOWN_ALL_PENDING: &str = "down-all timeout has not elapsed";
const DOWN_ALL_ELAPSED: &str = "down-all timeout elapsed";

/// Evaluates Split Brain Resolver settings against a downing context.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SplitBrainResolver {
  settings: SplitBrainResolverSettings,
}

impl SplitBrainResolver {
  /// Creates a resolver from immutable settings.
  #[must_use]
  pub const fn new(settings: SplitBrainResolverSettings) -> Self {
    Self { settings }
  }

  /// Returns the settings used by this resolver.
  #[must_use]
  pub const fn settings(&self) -> SplitBrainResolverSettings {
    self.settings
  }

  /// Evaluates the configured strategy without mutating membership state.
  #[must_use]
  pub fn decide(&self, context: &DowningDecisionContext) -> DowningStrategyDecision {
    let strategy = self.settings.active_strategy();

    if context.explicit_down_authority().is_some() {
      return DowningStrategyDecision::down(
        DowningDecisionTrace::majority_partition(strategy, String::from(EXPLICIT_DOWN_SELECTED)),
        Vec::new(),
      );
    }
    if strategy != SplitBrainResolverStrategy::DownAll
      && let Some(reason) = context.defer_reason()
    {
      return defer(strategy, reason);
    }
    if self.settings.stable_after() > Duration::ZERO && context.unstable_duration() < self.settings.stable_after() {
      return DowningStrategyDecision::defer(DowningDecisionTrace::stable_after_pending(
        strategy,
        self.settings.stable_after(),
        String::from(STABLE_AFTER_PENDING),
      ));
    }

    match strategy {
      | SplitBrainResolverStrategy::KeepMajority => {
        Self::decide_majority(context, strategy, REACHABLE_MAJORITY_SELECTED)
      },
      | SplitBrainResolverStrategy::LeaseMajority => Self::defer_lease_outcome(LeaseAcquisitionOutcome::BackendMissing),
      | SplitBrainResolverStrategy::StaticQuorum => self.decide_static_quorum(context),
      | SplitBrainResolverStrategy::KeepOldest => Self::decide_oldest(context),
      | SplitBrainResolverStrategy::DownAll => self.decide_down_all(context),
    }
  }

  /// Evaluates LeaseMajority with an explicit lease port.
  #[must_use]
  pub fn decide_with_lease(
    &self,
    context: &DowningDecisionContext,
    lease_port: &mut dyn LeaseMajorityPort,
  ) -> DowningStrategyDecision {
    if self.settings.active_strategy() != SplitBrainResolverStrategy::LeaseMajority {
      return self.decide(context);
    }
    if context.explicit_down_authority().is_some() {
      return DowningStrategyDecision::down(
        DowningDecisionTrace::majority_partition(
          SplitBrainResolverStrategy::LeaseMajority,
          String::from(EXPLICIT_DOWN_SELECTED),
        ),
        Vec::new(),
      );
    }
    if let Some(reason) = context.defer_reason() {
      return defer(SplitBrainResolverStrategy::LeaseMajority, reason);
    }
    if self.settings.stable_after() > Duration::ZERO && context.unstable_duration() < self.settings.stable_after() {
      return DowningStrategyDecision::defer(DowningDecisionTrace::stable_after_pending(
        SplitBrainResolverStrategy::LeaseMajority,
        self.settings.stable_after(),
        String::from(STABLE_AFTER_PENDING),
      ));
    }

    Self::decide_lease_majority(context, lease_port)
  }

  fn decide_majority(
    context: &DowningDecisionContext,
    strategy: SplitBrainResolverStrategy,
    retained_reason: &str,
  ) -> DowningStrategyDecision {
    let Some(partition) = PartitionEvaluation::from_context(context) else {
      return defer(strategy, MEMBERSHIP_REQUIRED);
    };
    if partition.active_count() == 0 {
      return defer(strategy, NO_ACTIVE_MEMBERS);
    }
    if partition.has_tie() {
      return DowningStrategyDecision::defer(
        DowningDecisionTrace::defer(strategy, String::from(MAJORITY_TIE)).with_tie_break(String::from(MAJORITY_TIE)),
      );
    }

    let threshold = partition.active_count() / 2 + 1;
    if partition.reachable.len() >= threshold {
      return keep_partition(strategy, retained_reason, partition.reachable, partition.non_reachable);
    }
    if partition.non_reachable.len() >= threshold {
      return keep_partition(strategy, NON_REACHABLE_MAJORITY_SELECTED, partition.non_reachable, partition.reachable);
    }

    defer(strategy, "no partition satisfies majority quorum")
  }

  fn decide_oldest(context: &DowningDecisionContext) -> DowningStrategyDecision {
    let strategy = SplitBrainResolverStrategy::KeepOldest;
    let Some(snapshot) = context.membership_snapshot() else {
      return defer(strategy, MEMBERSHIP_REQUIRED);
    };
    let active: Vec<NodeRecord> = snapshot.entries.iter().filter(|r| r.status.is_active()).cloned().collect();
    let Some(oldest) = oldest_member(active.as_slice()) else {
      return defer(strategy, NO_ACTIVE_MEMBERS);
    };
    let Some(partition) = PartitionEvaluation::from_context(context) else {
      return defer(strategy, MEMBERSHIP_REQUIRED);
    };

    if partition.reachable.iter().any(|member| member == &oldest.unique_address) {
      keep_partition(strategy, OLDEST_PARTITION_SELECTED, partition.reachable, partition.non_reachable)
    } else {
      keep_partition(strategy, OLDEST_PARTITION_SELECTED, partition.non_reachable, partition.reachable)
    }
  }

  fn decide_static_quorum(&self, context: &DowningDecisionContext) -> DowningStrategyDecision {
    let strategy = SplitBrainResolverStrategy::StaticQuorum;
    let Some(quorum_size) = self.settings.static_quorum_size() else {
      return defer(strategy, STATIC_QUORUM_SIZE_MISSING);
    };
    if quorum_size == 0 {
      return defer(strategy, STATIC_QUORUM_SIZE_ZERO);
    }
    let Some(partition) = PartitionEvaluation::from_context(context) else {
      return defer(strategy, MEMBERSHIP_REQUIRED);
    };
    if partition.active_count() == 0 {
      return defer(strategy, NO_ACTIVE_MEMBERS);
    }

    let reachable_satisfies_quorum = partition.reachable.len() >= quorum_size;
    let non_reachable_satisfies_quorum = partition.non_reachable.len() >= quorum_size;
    if reachable_satisfies_quorum && non_reachable_satisfies_quorum {
      return DowningStrategyDecision::defer(
        DowningDecisionTrace::defer(strategy, String::from(STATIC_QUORUM_TIE))
          .with_tie_break(String::from(STATIC_QUORUM_TIE)),
      );
    }

    if reachable_satisfies_quorum {
      return keep_partition(strategy, STATIC_QUORUM_SELECTED, partition.reachable, partition.non_reachable);
    }
    if non_reachable_satisfies_quorum {
      return keep_partition(
        strategy,
        NON_REACHABLE_STATIC_QUORUM_SELECTED,
        partition.non_reachable,
        partition.reachable,
      );
    }

    defer(strategy, "no partition satisfies static quorum")
  }

  fn decide_lease_majority(
    context: &DowningDecisionContext,
    lease_port: &mut dyn LeaseMajorityPort,
  ) -> DowningStrategyDecision {
    let strategy = SplitBrainResolverStrategy::LeaseMajority;
    let Some(partition) = PartitionEvaluation::from_context(context) else {
      return defer(strategy, MEMBERSHIP_REQUIRED);
    };
    if partition.active_count() == 0 {
      return defer(strategy, NO_ACTIVE_MEMBERS);
    }
    let has_tie = partition.has_tie();
    let threshold = partition.active_count() / 2 + 1;
    let (retained_partition, downing_targets) = if has_tie || partition.reachable.len() >= threshold {
      (partition.reachable, partition.non_reachable)
    } else if partition.non_reachable.len() >= threshold {
      (partition.non_reachable, partition.reachable)
    } else {
      return defer(strategy, "no partition satisfies majority quorum");
    };
    if context.reachability_observer().is_some_and(|observer| downing_targets.iter().any(|target| target == observer)) {
      return DowningStrategyDecision::down(
        DowningDecisionTrace::majority_partition(strategy, String::from(LOCAL_OBSERVER_DOWNING_TARGET)),
        downing_targets,
      );
    }

    match lease_port.acquire_majority(context) {
      | LeaseAcquisitionOutcome::Acquired => {
        DowningStrategyDecision::keep(lease_acquired_trace(strategy, has_tie), retained_partition, downing_targets)
      },
      | outcome @ (LeaseAcquisitionOutcome::Denied
      | LeaseAcquisitionOutcome::Unavailable
      | LeaseAcquisitionOutcome::Unknown
      | LeaseAcquisitionOutcome::BackendMissing) => Self::defer_lease_outcome(outcome),
    }
  }

  fn defer_lease_outcome(outcome: LeaseAcquisitionOutcome) -> DowningStrategyDecision {
    DowningStrategyDecision::defer(DowningDecisionTrace::from_lease_outcome(
      SplitBrainResolverStrategy::LeaseMajority,
      outcome,
    ))
  }

  fn decide_down_all(&self, context: &DowningDecisionContext) -> DowningStrategyDecision {
    let strategy = SplitBrainResolverStrategy::DownAll;
    let Some(snapshot) = context.membership_snapshot() else {
      return defer(strategy, MEMBERSHIP_REQUIRED);
    };
    let targets = active_member_addresses(snapshot.entries.as_slice());
    if targets.is_empty() {
      return defer(strategy, NO_ACTIVE_MEMBERS);
    }
    let timeout = self.settings.down_all_when_unstable();
    if timeout > Duration::ZERO && context.unstable_duration() < timeout {
      return DowningStrategyDecision::defer(DowningDecisionTrace::down_all_pending(
        strategy,
        timeout,
        String::from(DOWN_ALL_PENDING),
      ));
    }

    DowningStrategyDecision::all_down(
      DowningDecisionTrace::down_all_elapsed(strategy, timeout, String::from(DOWN_ALL_ELAPSED)),
      targets,
    )
  }
}

fn lease_acquired_trace(strategy: SplitBrainResolverStrategy, has_tie: bool) -> DowningDecisionTrace {
  let trace = DowningDecisionTrace::from_lease_outcome(strategy, LeaseAcquisitionOutcome::Acquired);
  if has_tie { trace.with_tie_break(String::from(MAJORITY_TIE)) } else { trace }
}

fn defer(strategy: SplitBrainResolverStrategy, reason: &str) -> DowningStrategyDecision {
  DowningStrategyDecision::defer(DowningDecisionTrace::defer(strategy, String::from(reason)))
}

fn keep_partition(
  strategy: SplitBrainResolverStrategy,
  reason: &str,
  retained_partition: Vec<UniqueAddress>,
  downing_targets: Vec<UniqueAddress>,
) -> DowningStrategyDecision {
  DowningStrategyDecision::keep(
    DowningDecisionTrace::majority_partition(strategy, String::from(reason)),
    retained_partition,
    downing_targets,
  )
}

fn active_member_addresses(records: &[NodeRecord]) -> Vec<UniqueAddress> {
  records.iter().filter(|record| record.status.is_active()).map(|record| record.unique_address.clone()).collect()
}

struct PartitionEvaluation {
  reachable:     Vec<UniqueAddress>,
  non_reachable: Vec<UniqueAddress>,
}

impl PartitionEvaluation {
  fn from_context(context: &DowningDecisionContext) -> Option<Self> {
    let snapshot = context.membership_snapshot()?;
    let mut reachable = Vec::new();
    let mut non_reachable = Vec::new();

    for record in snapshot.entries.iter().filter(|record| record.status.is_active()) {
      match context.reachability_status(&record.unique_address).unwrap_or(ReachabilityStatus::Reachable) {
        | ReachabilityStatus::Reachable => reachable.push(record.unique_address.clone()),
        | ReachabilityStatus::Unreachable | ReachabilityStatus::Terminated => {
          non_reachable.push(record.unique_address.clone());
        },
      }
    }

    Some(Self { reachable, non_reachable })
  }

  const fn active_count(&self) -> usize {
    self.reachable.len() + self.non_reachable.len()
  }

  const fn has_tie(&self) -> bool {
    self.reachable.len() == self.non_reachable.len()
  }
}
