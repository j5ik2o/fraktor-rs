//! Least-shard allocation strategy implementation.

use alloc::{
  collections::{BTreeMap, BTreeSet},
  string::String,
  vec::Vec,
};

use super::{RebalanceStrategySettings, ShardAllocationStrategy};

#[cfg(test)]
#[path = "least_shard_allocation_strategy_test.rs"]
mod tests;

/// Allocates shards to the region with the fewest shards and rebalances from overloaded regions.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LeastShardAllocationStrategy {
  settings: RebalanceStrategySettings,
}

impl LeastShardAllocationStrategy {
  /// Creates a strategy with the given rebalance limits.
  #[must_use]
  pub const fn new(settings: RebalanceStrategySettings) -> Self {
    Self { settings }
  }

  /// Creates a strategy with Pekko-compatible default limits.
  #[must_use]
  pub const fn with_defaults() -> Self {
    Self::new(RebalanceStrategySettings::new())
  }

  fn region_entries(current_allocations: &BTreeMap<String, Vec<String>>) -> Vec<(String, Vec<String>)> {
    current_allocations.iter().map(|(region, shards)| (region.clone(), shards.clone())).collect()
  }

  fn rebalance_phase1(
    number_of_shards: usize,
    optimal_per_region: usize,
    sorted_entries: &[(String, Vec<String>)],
    settings: &RebalanceStrategySettings,
  ) -> BTreeSet<String> {
    let mut selected = Vec::new();
    for (_, shards) in sorted_entries {
      if shards.len() > optimal_per_region {
        selected.extend(shards.iter().take(shards.len() - optimal_per_region).cloned());
      }
    }
    selected.into_iter().take(settings.rebalance_limit(number_of_shards)).collect()
  }

  fn rebalance_phase2(
    number_of_shards: usize,
    optimal_per_region: usize,
    sorted_entries: &[(String, Vec<String>)],
    settings: &RebalanceStrategySettings,
  ) -> BTreeSet<String> {
    let count_below_optimal: usize =
      sorted_entries.iter().map(|(_, shards)| optimal_per_region.saturating_sub(1).saturating_sub(shards.len())).sum();

    if count_below_optimal == 0 {
      return BTreeSet::new();
    }

    let mut selected = Vec::new();
    for (_, shards) in sorted_entries {
      if shards.len() >= optimal_per_region {
        if let Some(shard) = shards.first() {
          selected.push(shard.clone());
        }
      }
    }

    selected.into_iter().take(core::cmp::min(count_below_optimal, settings.rebalance_limit(number_of_shards))).collect()
  }
}

impl ShardAllocationStrategy for LeastShardAllocationStrategy {
  fn allocate_shard(
    &self,
    requester: &str,
    _shard_id: &str,
    current_allocations: &BTreeMap<String, Vec<String>>,
  ) -> Option<String> {
    if current_allocations.is_empty() {
      return Some(String::from(requester));
    }

    current_allocations
      .iter()
      .min_by(|(left_region, left_shards), (right_region, right_shards)| {
        left_shards.len().cmp(&right_shards.len()).then_with(|| left_region.cmp(right_region))
      })
      .map(|(region, _)| region.clone())
  }

  fn rebalance(
    &self,
    current_allocations: &BTreeMap<String, Vec<String>>,
    rebalance_in_progress: &BTreeSet<String>,
  ) -> BTreeSet<String> {
    if !rebalance_in_progress.is_empty() {
      return BTreeSet::new();
    }

    let mut sorted_entries = Self::region_entries(current_allocations);
    sorted_entries.sort_by(|(left_region, left_shards), (right_region, right_shards)| {
      left_shards.len().cmp(&right_shards.len()).then_with(|| left_region.cmp(right_region))
    });

    let number_of_shards: usize = sorted_entries.iter().map(|(_, shards)| shards.len()).sum();
    let number_of_regions = sorted_entries.len();
    if number_of_regions == 0 || number_of_shards == 0 {
      return BTreeSet::new();
    }

    let optimal_per_region =
      number_of_shards / number_of_regions + if number_of_shards.is_multiple_of(number_of_regions) { 0 } else { 1 };

    let phase1 = Self::rebalance_phase1(number_of_shards, optimal_per_region, &sorted_entries, &self.settings);
    if !phase1.is_empty() {
      return phase1;
    }

    Self::rebalance_phase2(number_of_shards, optimal_per_region, &sorted_entries, &self.settings)
  }
}
