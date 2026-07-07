//! External shard allocation strategy port and location snapshot.

use alloc::{
  collections::{BTreeMap, BTreeSet},
  string::String,
  vec::Vec,
};

#[cfg(test)]
#[path = "shard_allocation_strategy_test.rs"]
mod tests;

/// Snapshot of externally supplied shard locations.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ExternalShardLocations {
  locations: BTreeMap<String, String>,
}

impl ExternalShardLocations {
  /// Creates an empty external shard location snapshot.
  #[must_use]
  pub fn new() -> Self {
    Self { locations: BTreeMap::new() }
  }

  /// Creates a snapshot from shard-to-region mappings.
  #[must_use]
  pub fn from_locations(locations: BTreeMap<String, String>) -> Self {
    Self { locations }
  }

  /// Inserts or replaces one shard location.
  pub fn insert(&mut self, shard_id: impl Into<String>, region_address: impl Into<String>) {
    self.locations.insert(shard_id.into(), region_address.into());
  }

  /// Returns the configured shard locations.
  #[must_use]
  pub fn locations(&self) -> &BTreeMap<String, String> {
    &self.locations
  }

  /// Returns the region address for a shard, if configured.
  #[must_use]
  pub fn region_for_shard(&self, shard_id: &str) -> Option<&str> {
    self.locations.get(shard_id).map(String::as_str)
  }
}

/// Pluggable shard allocation and rebalancing strategy.
///
/// Implementations decide where new shards are allocated and which shards should
/// be rebalanced during cluster membership changes.
pub trait ShardAllocationStrategy {
  /// Chooses the region address responsible for a newly allocated shard.
  ///
  /// `current_allocations` maps region addresses to the shard identifiers they
  /// currently host, in allocation order.
  fn allocate_shard(
    &self,
    requester: &str,
    shard_id: &str,
    current_allocations: &BTreeMap<String, Vec<String>>,
  ) -> Option<String>;

  /// Returns shard identifiers that should be rebalanced away from their current region.
  ///
  /// `rebalance_in_progress` contains shards already being migrated and must not be
  /// returned again.
  fn rebalance(
    &self,
    current_allocations: &BTreeMap<String, Vec<String>>,
    rebalance_in_progress: &BTreeSet<String>,
  ) -> BTreeSet<String>;
}
