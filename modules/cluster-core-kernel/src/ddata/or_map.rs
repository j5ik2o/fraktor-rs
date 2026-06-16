//! Observed-remove map CRDT whose values are merged as CRDTs.

#[cfg(test)]
#[path = "or_map_test.rs"]
mod tests;

use alloc::collections::{BTreeMap, BTreeSet};

use fraktor_remote_core_rs::address::UniqueAddress;

use super::{
  DeltaReplicatedData, ORSet, RemovedNodePruning, ReplicatedData, ReplicatedDelta, RequiresCausalDeliveryOfDeltas,
  SelfUniqueAddress, VersionVector, or_set::pruning_node_for,
};

/// Observed-remove map CRDT, also known as OR-Map.
///
/// Keys are tracked with observed-remove semantics (an [`ORSet`]); concurrent additions of a key
/// win over removals. Values must be [`ReplicatedData`] and are merged recursively when the same
/// key is updated concurrently. `put` replaces a value and does not preserve its causal history, so
/// it must not be used to replace an observed-remove collection value; use `update` or `ORMultiMap`
/// for that case. Equality ignores the local delta marker; the delta is the accumulated full state
/// since the last reset.
#[derive(Debug, Clone)]
pub struct ORMap<A, B> {
  keys:        ORSet<A>,
  values:      BTreeMap<A, B>,
  delta_dirty: bool,
}

impl<A, B> ORMap<A, B>
where
  A: Clone + Ord,
  B: ReplicatedData,
{
  /// Creates an empty map.
  #[must_use]
  pub const fn new() -> Self {
    Self { keys: ORSet::new(), values: BTreeMap::new(), delta_dirty: false }
  }

  /// Returns a map with `value` stored at `key`, replacing any existing value.
  ///
  /// The previous value's causal history is not preserved. Use `update` to merge with the existing
  /// value, and `ORMultiMap` for set-valued maps.
  #[must_use]
  pub fn put(&self, node: &SelfUniqueAddress, key: A, value: B) -> Self {
    let keys = if self.keys.contains(&key) {
      self.keys.remove(&key).add(node, key.clone())
    } else {
      self.keys.add(node, key.clone())
    };
    let mut values = self.values.clone();
    values.insert(key, value);
    Self { keys, values, delta_dirty: true }
  }

  /// Returns a map with the value at `key` replaced by applying `modify`.
  ///
  /// When `key` is absent, `modify` is applied to `initial`.
  #[must_use]
  pub fn update(&self, node: &SelfUniqueAddress, key: A, initial: B, modify: impl FnOnce(B) -> B) -> Self {
    let base = match self.values.get(&key) {
      | Some(existing) => existing.clone(),
      | None => initial,
    };
    let new_value = modify(base);
    let keys = self.keys.add(node, key.clone());
    let mut values = self.values.clone();
    values.insert(key, new_value);
    Self { keys, values, delta_dirty: true }
  }

  /// Returns a map with the observed entry for `key` removed.
  ///
  /// A concurrent update on another replica that this replica has not observed keeps the entry
  /// after a later merge.
  #[must_use]
  pub fn remove(&self, key: &A) -> Self {
    if !self.values.contains_key(key) {
      return self.clone();
    }

    let keys = self.keys.remove(key);
    let mut values = self.values.clone();
    values.remove(key);
    Self { keys, values, delta_dirty: true }
  }

  /// Returns the value associated with `key`, or `None` when absent.
  #[must_use]
  pub fn get(&self, key: &A) -> Option<&B> {
    self.values.get(key)
  }

  /// Returns the visible entries.
  #[must_use]
  pub const fn entries(&self) -> &BTreeMap<A, B> {
    &self.values
  }

  /// Returns true when `key` has a visible value.
  #[must_use]
  pub fn contains_key(&self, key: &A) -> bool {
    self.values.contains_key(key)
  }

  /// Returns true when the map has no visible entries.
  #[must_use]
  pub fn is_empty(&self) -> bool {
    self.values.is_empty()
  }

  /// Returns the number of visible entries.
  #[must_use]
  pub fn len(&self) -> usize {
    self.values.len()
  }
}

impl<A, B> ReplicatedData for ORMap<A, B>
where
  A: Clone + Ord,
  B: ReplicatedData,
{
  fn merge(&self, other: &Self) -> Self {
    let keys = self.keys.merge(&other.keys);
    let mut values = BTreeMap::new();

    for key in keys.elements() {
      let left_dots = contributing_key_dots(&self.keys, &key, &keys);
      let right_dots = contributing_key_dots(&other.keys, &key, &keys);
      let left_contributes = !left_dots.is_empty();
      let right_contributes = !right_dots.is_empty();
      match (self.values.get(&key), other.values.get(&key), left_contributes, right_contributes) {
        | (Some(left), Some(right), true, true) => {
          let value = if covered_by_pruned_aliases(&right_dots, &left_dots) {
            left.clone()
          } else if covered_by_pruned_aliases(&left_dots, &right_dots) {
            right.clone()
          } else {
            left.merge(right)
          };
          values.insert(key, value);
        },
        | (Some(left), _, true, _) => {
          values.insert(key, left.clone());
        },
        | (_, Some(right), _, true) => {
          values.insert(key, right.clone());
        },
        | _ => {},
      }
    }

    Self { keys, values, delta_dirty: self.delta_dirty }
  }
}

impl<A, B> DeltaReplicatedData for ORMap<A, B>
where
  A: Clone + Ord,
  B: ReplicatedData,
{
  type Delta = Self;

  fn delta(&self) -> Option<Self::Delta> {
    if self.delta_dirty {
      Some(Self { keys: self.keys.clone(), values: self.values.clone(), delta_dirty: false })
    } else {
      None
    }
  }

  fn merge_delta(&self, delta: &Self::Delta) -> Self {
    self.merge(delta)
  }

  fn reset_delta(&self) -> Self {
    Self { keys: self.keys.clone(), values: self.values.clone(), delta_dirty: false }
  }
}

impl<A, B> ReplicatedDelta for ORMap<A, B>
where
  A: Clone + Ord,
  B: ReplicatedData,
{
  type Full = Self;

  fn zero(&self) -> Self::Full {
    let _ = self;
    Self::new()
  }
}

impl<A, B> RequiresCausalDeliveryOfDeltas for ORMap<A, B>
where
  A: Clone + Ord,
  B: ReplicatedData,
{
}

impl<A, B> RemovedNodePruning for ORMap<A, B>
where
  A: Clone + Ord,
  B: RemovedNodePruning,
{
  type PruneError = B::PruneError;

  fn modified_by_nodes(&self) -> BTreeSet<UniqueAddress> {
    let mut nodes = self.keys.modified_by_nodes();
    for value in self.values.values() {
      nodes.extend(value.modified_by_nodes());
    }
    nodes
  }

  fn need_pruning_from(&self, removed_node: &UniqueAddress) -> bool {
    self.keys.need_pruning_from(removed_node) || self.values.values().any(|value| value.need_pruning_from(removed_node))
  }

  fn prune(&self, removed_node: &UniqueAddress, collapse_into: &UniqueAddress) -> Result<Self, Self::PruneError> {
    let keys = match self.keys.prune(removed_node, collapse_into) {
      | Ok(keys) => keys,
      | Err(never) => match never {},
    };
    let mut delta_dirty = self.delta_dirty || keys != self.keys;

    let mut values = BTreeMap::new();
    for (key, value) in &self.values {
      if value.need_pruning_from(removed_node) {
        values.insert(key.clone(), value.prune(removed_node, collapse_into)?);
        delta_dirty = true;
      } else {
        values.insert(key.clone(), value.clone());
      }
    }

    Ok(Self { keys, values, delta_dirty })
  }

  fn pruning_cleanup(&self, removed_node: &UniqueAddress) -> Self {
    let keys = self.keys.pruning_cleanup(removed_node);

    let mut values = BTreeMap::new();
    let mut values_dirty = false;
    for (key, value) in &self.values {
      if !keys.contains(key) {
        values_dirty = true;
        continue;
      }
      if value.need_pruning_from(removed_node) {
        values.insert(key.clone(), value.pruning_cleanup(removed_node));
        values_dirty = true;
      } else {
        values.insert(key.clone(), value.clone());
      }
    }

    let delta_dirty = self.delta_dirty || keys != self.keys || values_dirty;
    Self { keys, values, delta_dirty }
  }
}

impl<A, B> Default for ORMap<A, B>
where
  A: Clone + Ord,
  B: ReplicatedData,
{
  fn default() -> Self {
    Self::new()
  }
}

impl<A, B> PartialEq for ORMap<A, B>
where
  A: Ord,
  B: PartialEq,
{
  fn eq(&self, other: &Self) -> bool {
    self.keys == other.keys && self.values == other.values
  }
}

impl<A, B> Eq for ORMap<A, B>
where
  A: Ord,
  B: Eq,
{
}

fn contributing_key_dots<A>(side_keys: &ORSet<A>, key: &A, merged_keys: &ORSet<A>) -> VersionVector
where
  A: Clone + Ord, {
  let Some(side_dots) = side_keys.dots_for(key) else {
    return VersionVector::new();
  };
  let Some(merged_dots) = merged_keys.dots_for(key) else {
    return VersionVector::new();
  };
  matching_dots(side_dots, merged_dots)
}

fn matching_dots(left: &VersionVector, right: &VersionVector) -> VersionVector {
  VersionVector::from_entries(
    left
      .entries()
      .filter(|(node, version)| right.version_at(node) == *version)
      .map(|(node, version)| (node.clone(), version)),
  )
}

fn covered_by_pruned_aliases(dots: &VersionVector, pruned_dots: &VersionVector) -> bool {
  !dots.is_empty() && dots.entries().all(|(node, version)| has_pruned_alias(node, version, pruned_dots))
}

fn has_pruned_alias(node: &UniqueAddress, version: u64, pruned_dots: &VersionVector) -> bool {
  pruned_dots
    .entries()
    .any(|(pruned_node, pruned_version)| pruning_node_for(node) == *pruned_node && pruned_version >= version)
}
