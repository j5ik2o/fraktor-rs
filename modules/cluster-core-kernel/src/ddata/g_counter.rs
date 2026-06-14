//! Grow-only counter CRDT.

#[cfg(test)]
#[path = "g_counter_test.rs"]
mod tests;

use alloc::collections::{BTreeMap, BTreeSet};

use fraktor_remote_core_rs::address::UniqueAddress;

use super::{
  CounterArithmeticError, DeltaReplicatedData, RemovedNodePruning, ReplicatedData, ReplicatedDelta, SelfUniqueAddress,
};

/// Grow-only counter CRDT with per-node slots.
#[derive(Debug, Clone)]
pub struct GCounter {
  state: BTreeMap<UniqueAddress, u128>,
  delta: BTreeMap<UniqueAddress, u128>,
}

impl GCounter {
  /// Creates an empty counter.
  #[must_use]
  pub const fn new() -> Self {
    Self::from_parts(BTreeMap::new(), BTreeMap::new())
  }

  pub(super) const fn from_parts(state: BTreeMap<UniqueAddress, u128>, delta: BTreeMap<UniqueAddress, u128>) -> Self {
    Self { state, delta }
  }

  pub(super) fn retain_nodes(&self, nodes: &BTreeMap<UniqueAddress, u64>) -> Self {
    let state = self
      .state
      .iter()
      .filter(|(node, _)| nodes.contains_key(*node))
      .map(|(node, value)| (node.clone(), *value))
      .collect();
    let delta = self
      .delta
      .iter()
      .filter(|(node, _)| nodes.contains_key(*node))
      .map(|(node, value)| (node.clone(), *value))
      .collect();

    Self { state, delta }
  }

  /// Returns a counter with `n` added to the local node slot.
  ///
  /// # Errors
  ///
  /// Returns [`CounterArithmeticError::Overflow`] when the node slot cannot represent the new
  /// value.
  pub fn increment(&self, node: &SelfUniqueAddress, n: u64) -> Result<Self, CounterArithmeticError> {
    if n == 0 {
      return Ok(self.clone());
    }

    let unique_address = node.unique_address();
    let current = self.state.get(unique_address).copied().unwrap_or(0);
    let next = current.checked_add(u128::from(n)).ok_or(CounterArithmeticError::Overflow)?;

    let mut state = self.state.clone();
    state.insert(unique_address.clone(), next);

    let mut delta = self.delta.clone();
    delta.insert(unique_address.clone(), next);

    Ok(Self { state, delta })
  }

  /// Returns the sum of all node slots.
  ///
  /// # Errors
  ///
  /// Returns [`CounterArithmeticError::Overflow`] when the sum exceeds `u128`.
  pub fn value(&self) -> Result<u128, CounterArithmeticError> {
    checked_sum(self.state.values().copied())
  }
}

impl ReplicatedData for GCounter {
  fn merge(&self, other: &Self) -> Self {
    Self { state: merge_state(&self.state, &other.state), delta: self.delta.clone() }
  }
}

impl DeltaReplicatedData for GCounter {
  type Delta = Self;

  fn delta(&self) -> Option<Self::Delta> {
    if self.delta.is_empty() { None } else { Some(Self { state: self.delta.clone(), delta: BTreeMap::new() }) }
  }

  fn merge_delta(&self, delta: &Self::Delta) -> Self {
    Self { state: merge_state(&self.state, &delta.state), delta: self.delta.clone() }
  }

  fn reset_delta(&self) -> Self {
    Self { state: self.state.clone(), delta: BTreeMap::new() }
  }
}

impl ReplicatedDelta for GCounter {
  type Full = Self;

  fn zero(&self) -> Self::Full {
    let _ = self;
    Self::new()
  }
}

impl RemovedNodePruning for GCounter {
  type PruneError = CounterArithmeticError;

  fn modified_by_nodes(&self) -> BTreeSet<UniqueAddress> {
    self.state.keys().cloned().collect()
  }

  fn need_pruning_from(&self, removed_node: &UniqueAddress) -> bool {
    self.state.contains_key(removed_node)
  }

  fn prune(&self, removed_node: &UniqueAddress, collapse_into: &UniqueAddress) -> Result<Self, Self::PruneError> {
    let Some(removed_value) = self.state.get(removed_node).copied() else {
      return Ok(self.clone());
    };

    let mut state = self.state.clone();
    state.remove(removed_node);

    let mut delta = self.delta.clone();
    delta.remove(removed_node);

    if removed_node == collapse_into {
      return Ok(Self { state, delta });
    }

    let current = state.get(collapse_into).copied().unwrap_or(0);
    let next = current.checked_add(removed_value).ok_or(CounterArithmeticError::Overflow)?;
    state.insert(collapse_into.clone(), next);
    delta.insert(collapse_into.clone(), next);

    Ok(Self { state, delta })
  }

  fn pruning_cleanup(&self, removed_node: &UniqueAddress) -> Self {
    let mut state = self.state.clone();
    state.remove(removed_node);

    let mut delta = self.delta.clone();
    delta.remove(removed_node);

    Self { state, delta }
  }
}

impl Default for GCounter {
  fn default() -> Self {
    Self::new()
  }
}

impl PartialEq for GCounter {
  fn eq(&self, other: &Self) -> bool {
    self.state == other.state
  }
}

impl Eq for GCounter {}

fn checked_sum(mut values: impl Iterator<Item = u128>) -> Result<u128, CounterArithmeticError> {
  values.try_fold(0_u128, |sum, value| sum.checked_add(value).ok_or(CounterArithmeticError::Overflow))
}

fn merge_state(
  left: &BTreeMap<UniqueAddress, u128>,
  right: &BTreeMap<UniqueAddress, u128>,
) -> BTreeMap<UniqueAddress, u128> {
  let mut merged = left.clone();

  for (node, value) in right {
    let current = merged.get(node).copied().unwrap_or(0);
    if *value > current {
      merged.insert(node.clone(), *value);
    }
  }

  merged
}
