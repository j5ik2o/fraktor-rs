//! Grow-only map of positive-negative counters.

#[cfg(test)]
#[path = "pn_counter_map_test.rs"]
mod tests;

use alloc::collections::{BTreeMap, BTreeSet};

use fraktor_remote_core_rs::address::UniqueAddress;

use super::{
  CounterArithmeticError, DeltaReplicatedData, PNCounter, RemovedNodePruning, ReplicatedData, ReplicatedDelta,
  SelfUniqueAddress,
};

/// CRDT map whose values are positive-negative counters.
#[derive(Debug, Clone)]
pub struct PNCounterMap<K> {
  entries: BTreeMap<K, PNCounter>,
  delta:   BTreeMap<K, PNCounter>,
}

impl<K> PNCounterMap<K>
where
  K: Ord + Clone,
{
  /// Creates an empty counter map.
  #[must_use]
  pub const fn new() -> Self {
    Self { entries: BTreeMap::new(), delta: BTreeMap::new() }
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
    if n == 0 {
      return Ok(self.clone());
    }

    let current = self.entries.get(&key).cloned().unwrap_or_else(PNCounter::new);
    let next = update(&current, node, n)?;

    let mut entries = self.entries.clone();
    entries.insert(key.clone(), next.clone());

    let mut delta = self.delta.clone();
    delta.insert(key, next.delta().unwrap_or_default());

    Ok(Self { entries, delta })
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
        .or_insert_with(|| counter.reset_delta());
    }

    Self { entries, delta: self.delta.clone() }
  }
}

impl<K> DeltaReplicatedData for PNCounterMap<K>
where
  K: Ord + Clone,
{
  type Delta = Self;

  fn delta(&self) -> Option<Self::Delta> {
    if self.delta.is_empty() { None } else { Some(Self { entries: self.delta.clone(), delta: BTreeMap::new() }) }
  }

  fn merge_delta(&self, delta: &Self::Delta) -> Self {
    let mut entries = self.entries.clone();

    for (key, counter) in &delta.entries {
      entries
        .entry(key.clone())
        .and_modify(|current| *current = current.merge_delta(counter))
        .or_insert_with(|| counter.reset_delta());
    }

    Self { entries, delta: self.delta.clone() }
  }

  fn reset_delta(&self) -> Self {
    let entries = self.entries.iter().map(|(key, counter)| (key.clone(), counter.reset_delta())).collect();

    Self { entries, delta: BTreeMap::new() }
  }
}

impl<K> ReplicatedDelta for PNCounterMap<K>
where
  K: Ord + Clone,
{
  type Full = Self;

  fn zero(&self) -> Self::Full {
    let _ = self;
    Self::new()
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

    Ok(Self { entries, delta: BTreeMap::new() })
  }

  fn pruning_cleanup(&self, removed_node: &UniqueAddress) -> Self {
    let entries =
      self.entries.iter().map(|(key, counter)| (key.clone(), counter.pruning_cleanup(removed_node))).collect();
    let delta = self
      .delta
      .iter()
      .filter_map(|(key, counter)| {
        let counter = counter.pruning_cleanup(removed_node);
        if counter.modified_by_nodes().is_empty() { None } else { Some((key.clone(), counter)) }
      })
      .collect();

    Self { entries, delta }
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

impl<K> PartialEq for PNCounterMap<K>
where
  K: Ord,
{
  fn eq(&self, other: &Self) -> bool {
    self.entries == other.entries
  }
}

impl<K> Eq for PNCounterMap<K> where K: Ord {}
