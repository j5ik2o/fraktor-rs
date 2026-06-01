//! Split Brain Resolver evaluator.

#[cfg(test)]
#[path = "split_brain_resolver_test.rs"]
mod tests;

use alloc::{string::String, vec::Vec};
use core::time::Duration;

use fraktor_remote_core_rs::address::UniqueAddress;

use super::{
  DowningDecisionContext, DowningDecisionTrace, DowningStrategyDecision, SplitBrainResolverSettings,
  SplitBrainResolverStrategy,
};
use crate::membership::{NodeRecord, ReachabilityStatus};

const MEMBERSHIP_REQUIRED: &str = "membership snapshot is required for split-brain evaluation";
const NO_ACTIVE_MEMBERS: &str = "no active members are available for split-brain evaluation";
const REACHABLE_MAJORITY_SELECTED: &str = "reachable majority partition selected";
const NON_REACHABLE_MAJORITY_SELECTED: &str = "non-reachable majority partition selected";
const STATIC_QUORUM_SELECTED: &str = "reachable static quorum partition selected";
const OLDEST_PARTITION_SELECTED: &str = "oldest member partition selected";
const MAJORITY_TIE: &str = "reachable and non-reachable partitions have equal size";
const STABLE_AFTER_PENDING: &str = "membership has not been stable for the required duration";
const DOWN_ALL_PENDING: &str = "down-all timeout has not elapsed";
const DOWN_ALL_ELAPSED: &str = "down-all timeout elapsed";
const LEASE_BACKEND_MISSING: &str = "lease majority port is not configured";

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

    if let Some(reason) = context.defer_reason() {
      return defer(strategy, reason);
    }
    if self.settings.stable_after() > Duration::ZERO {
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
      | SplitBrainResolverStrategy::LeaseMajority => defer(strategy, LEASE_BACKEND_MISSING),
      | SplitBrainResolverStrategy::StaticQuorum => Self::decide_majority(context, strategy, STATIC_QUORUM_SELECTED),
      | SplitBrainResolverStrategy::KeepOldest => Self::decide_oldest(context),
      | SplitBrainResolverStrategy::DownAll => self.decide_down_all(context),
    }
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
    let Some(oldest) = oldest_active_member(snapshot.entries.as_slice()) else {
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

  fn decide_down_all(&self, context: &DowningDecisionContext) -> DowningStrategyDecision {
    let strategy = SplitBrainResolverStrategy::DownAll;
    let Some(partition) = PartitionEvaluation::from_context(context) else {
      return defer(strategy, MEMBERSHIP_REQUIRED);
    };
    if partition.active_count() == 0 {
      return defer(strategy, NO_ACTIVE_MEMBERS);
    }
    let targets = partition.all_active_members();
    let timeout = self.settings.down_all_when_unstable();
    if timeout > Duration::ZERO {
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

fn oldest_active_member(records: &[NodeRecord]) -> Option<&NodeRecord> {
  let mut oldest = None;
  for record in records.iter().filter(|record| record.status.is_active()) {
    oldest = match oldest {
      | Some(current) if !record.is_older_than(current) => Some(current),
      | _ => Some(record),
    };
  }
  oldest
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

  fn all_active_members(self) -> Vec<UniqueAddress> {
    self.reachable.into_iter().chain(self.non_reachable).collect()
  }
}
