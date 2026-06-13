//! Grow-only map of positive-negative counters.

#[cfg(test)]
#[path = "pn_counter_map_test.rs"]
mod tests;

use alloc::collections::{BTreeMap, BTreeSet};

use fraktor_remote_core_rs::address::UniqueAddress;

use super::{CounterArithmeticError, PNCounter, RemovedNodePruning, ReplicatedData, SelfUniqueAddress};

/// CRDT map whose values are positive-negative counters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PNCounterMap<K> {
  entries: BTreeMap<K, PNCounter>,
}

impl<K> PNCounterMap<K>
where
  K: Ord + Clone,
{
  /// Creates an empty counter map.
  #[must_use]
  pub const fn new() -> Self {
    Self { entries: BTreeMap::new() }
  }

  /// Returns a map with `n` added to the counter at `key`.
  ///
  /// # Errors
  ///
  /// Returns [`CounterArithmeticError::Overflow`] when the nested counter cannot represent the new
  /// value.
  pub fn increment(&self, node: &SelfUniqueAddress, key: K, n: u64) -> Result<Self, CounterArithmeticError> {
    self.update_key(node, key, n, PNCounter::increment)
  }

  /// Returns a map with `n` subtracted from the counter at `key`.
  ///
  /// # Errors
  ///
  /// Returns [`CounterArithmeticError::Overflow`] when the nested counter cannot represent the new
  /// value.
  pub fn decrement(&self, node: &SelfUniqueAddress, key: K, n: u64) -> Result<Self, CounterArithmeticError> {
    self.update_key(node, key, n, PNCounter::decrement)
  }

  /// Returns the current value for `key`, or `None` when the key is absent.
  ///
  /// # Errors
  ///
  /// Returns [`CounterArithmeticError::Overflow`] when the nested counter value cannot fit in
  /// `i128`.
  pub fn get(&self, key: &K) -> Result<Option<i128>, CounterArithmeticError> {
    self.entries.get(key).map(PNCounter::value).transpose()
  }

  fn update_key(
    &self,
    node: &SelfUniqueAddress,
    key: K,
    n: u64,
    update: fn(&PNCounter, &SelfUniqueAddress, u64) -> Result<PNCounter, CounterArithmeticError>,
  ) -> Result<Self, CounterArithmeticError> {
    if n == 0 && !self.entries.contains_key(&key) {
      return Ok(self.clone());
    }

    let current = self.entries.get(&key).cloned().unwrap_or_else(PNCounter::new);
    let next = update(&current, node, n)?;

    let mut entries = self.entries.clone();
    entries.insert(key, next);

    Ok(Self { entries })
  }
}

impl<K> ReplicatedData for PNCounterMap<K>
where
  K: Ord + Clone,
{
  fn merge(&self, other: &Self) -> Self {
    let mut entries = self.entries.clone();

    for (key, counter) in &other.entries {
      entries
        .entry(key.clone())
        .and_modify(|current| *current = current.merge(counter))
        .or_insert_with(|| counter.clone());
    }

    Self { entries }
  }
}

impl<K> RemovedNodePruning for PNCounterMap<K>
where
  K: Ord + Clone,
{
  type PruneError = CounterArithmeticError;

  fn modified_by_nodes(&self) -> BTreeSet<UniqueAddress> {
    let mut nodes = BTreeSet::new();
    for counter in self.entries.values() {
      nodes.extend(counter.modified_by_nodes());
    }
    nodes
  }

  fn need_pruning_from(&self, removed_node: &UniqueAddress) -> bool {
    self.entries.values().any(|counter| counter.need_pruning_from(removed_node))
  }

  fn prune(&self, removed_node: &UniqueAddress, collapse_into: &UniqueAddress) -> Result<Self, Self::PruneError> {
    let mut entries = BTreeMap::new();
    for (key, counter) in &self.entries {
      entries.insert(key.clone(), counter.prune(removed_node, collapse_into)?);
    }

    Ok(Self { entries })
  }

  fn pruning_cleanup(&self, removed_node: &UniqueAddress) -> Self {
    let entries =
      self.entries.iter().map(|(key, counter)| (key.clone(), counter.pruning_cleanup(removed_node))).collect();

    Self { entries }
  }
}

impl<K> Default for PNCounterMap<K>
where
  K: Ord + Clone,
{
  fn default() -> Self {
    Self::new()
  }
}
