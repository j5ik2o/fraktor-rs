//! Grow-only map of positive-negative counters.

#[cfg(test)]
#[path = "pn_counter_map_test.rs"]
mod tests;

use alloc::collections::{BTreeMap, BTreeSet};

use fraktor_remote_core_rs::address::UniqueAddress;

use super::{
  CounterArithmeticError, DeltaReplicatedData, PNCounter, RemovedNodePruning, ReplicatedData, ReplicatedDelta,
  RequiresCausalDeliveryOfDeltas, SelfUniqueAddress,
};

/// CRDT map whose values are positive-negative counters.
///
/// Equality ignores local delta buffers but includes causal dot and tombstone metadata because
/// that state affects future merges.
#[derive(Debug, Clone)]
pub struct PNCounterMap<K> {
  // Invariant: every visible entry has a matching dot map; merge derives candidates from dots.
  entries:              BTreeMap<K, PNCounter>,
  dots:                 BTreeMap<K, BTreeMap<UniqueAddress, u64>>,
  removed_dots:         BTreeMap<K, BTreeMap<UniqueAddress, u64>>,
  removed_values:       BTreeMap<K, PNCounter>,
  delta:                BTreeMap<K, PNCounter>,
  delta_dots:           BTreeMap<K, BTreeMap<UniqueAddress, u64>>,
  delta_removed_dots:   BTreeMap<K, BTreeMap<UniqueAddress, u64>>,
  delta_removed_values: BTreeMap<K, PNCounter>,
}

impl<K> PNCounterMap<K>
where
  K: Ord + Clone,
{
  /// Creates an empty counter map.
  #[must_use]
  pub const fn new() -> Self {
    Self {
      entries:              BTreeMap::new(),
      dots:                 BTreeMap::new(),
      removed_dots:         BTreeMap::new(),
      removed_values:       BTreeMap::new(),
      delta:                BTreeMap::new(),
      delta_dots:           BTreeMap::new(),
      delta_removed_dots:   BTreeMap::new(),
      delta_removed_values: BTreeMap::new(),
    }
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

  /// Returns the visible entries as signed counter values.
  ///
  /// # Errors
  ///
  /// Returns [`CounterArithmeticError::Overflow`] when any nested counter value cannot fit in
  /// `i128`.
  pub fn entries(&self) -> Result<BTreeMap<K, i128>, CounterArithmeticError> {
    self.entries.iter().map(|(key, counter)| Ok((key.clone(), counter.value()?))).collect()
  }

  /// Returns true when `key` has a visible counter entry.
  #[must_use]
  pub fn contains_key(&self, key: &K) -> bool {
    self.entries.contains_key(key)
  }

  /// Returns true when the map has no visible counter entries.
  #[must_use]
  pub fn is_empty(&self) -> bool {
    self.entries.is_empty()
  }

  /// Returns the number of visible counter entries.
  #[must_use]
  pub fn len(&self) -> usize {
    self.entries.len()
  }

  /// Returns a map with the observed entry for `key` removed.
  ///
  /// A concurrent update that has not been observed by this replica survives a later merge.
  #[must_use]
  pub fn remove(&self, key: &K) -> Self {
    if !self.entries.contains_key(key) {
      return self.clone();
    }

    let Some(observed_dots) = self.dots.get(key).cloned() else {
      return self.clone();
    };

    let mut entries = self.entries.clone();
    entries.remove(key);

    let mut dots = self.dots.clone();
    dots.remove(key);

    let mut removed_dots = self.removed_dots.clone();
    merge_dot_map_entry(&mut removed_dots, key.clone(), &observed_dots);

    let observed_value = self.entries.get(key).map(|counter| counter.retain_nodes(&observed_dots).reset_delta());
    let mut removed_values = self.removed_values.clone();
    if let Some(observed_value) = &observed_value {
      merge_counter_map_entry(&mut removed_values, key.clone(), observed_value.clone());
    }

    let mut delta_removed_dots = self.delta_removed_dots.clone();
    merge_dot_map_entry(&mut delta_removed_dots, key.clone(), &observed_dots);

    let mut delta_removed_values = self.delta_removed_values.clone();
    if let Some(observed_value) = observed_value {
      merge_counter_map_entry(&mut delta_removed_values, key.clone(), observed_value);
    }

    let mut delta = self.delta.clone();
    let mut delta_dots = self.delta_dots.clone();
    apply_removed_dots(&mut delta, &mut delta_dots, key, &observed_dots, None, removed_values.get(key));

    Self { entries, dots, removed_dots, removed_values, delta, delta_dots, delta_removed_dots, delta_removed_values }
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

    let mut dots = self.dots.clone();
    let mut key_dots = dots.remove(&key).unwrap_or_default();
    let node_dot = node.unique_address().clone();
    let current_dot = key_dots.get(&node_dot).copied().unwrap_or(0);
    let removed_dot =
      self.removed_dots.get(&key).and_then(|removed_key_dots| removed_key_dots.get(&node_dot)).copied().unwrap_or(0);
    key_dots.insert(node_dot, current_dot.max(removed_dot).saturating_add(1));
    dots.insert(key.clone(), key_dots.clone());

    let mut delta = self.delta.clone();
    delta.insert(key.clone(), next.reset_delta());

    let mut delta_dots = self.delta_dots.clone();
    delta_dots.insert(key, key_dots);

    Ok(Self {
      entries,
      dots,
      removed_dots: self.removed_dots.clone(),
      removed_values: self.removed_values.clone(),
      delta,
      delta_dots,
      delta_removed_dots: self.delta_removed_dots.clone(),
      delta_removed_values: self.delta_removed_values.clone(),
    })
  }
}

impl<K> ReplicatedData for PNCounterMap<K>
where
  K: Ord + Clone,
{
  fn merge(&self, other: &Self) -> Self {
    let removed_dots = merge_dot_maps(&self.removed_dots, &other.removed_dots);
    let removed_values = merge_counter_maps(&self.removed_values, &other.removed_values);
    let (entries, dots) = merge_entries(
      &MergeEntrySide {
        entries:                 &self.entries,
        dots:                    &self.dots,
        own_removed:             &self.removed_dots,
        removed_by_other:        &other.removed_dots,
        removed_values_by_other: &other.removed_values,
      },
      &MergeEntrySide {
        entries:                 &other.entries,
        dots:                    &other.dots,
        own_removed:             &other.removed_dots,
        removed_by_other:        &self.removed_dots,
        removed_values_by_other: &self.removed_values,
      },
    );
    let mut delta = self.delta.clone();
    let mut delta_dots = self.delta_dots.clone();
    for (key, removed_key_dots) in &other.removed_dots {
      apply_removed_dots(
        &mut delta,
        &mut delta_dots,
        key,
        removed_key_dots,
        self.removed_dots.get(key),
        removed_values.get(key),
      );
    }

    Self {
      entries,
      dots,
      removed_dots,
      removed_values,
      delta,
      delta_dots,
      delta_removed_dots: self.delta_removed_dots.clone(),
      delta_removed_values: self.delta_removed_values.clone(),
    }
  }
}

impl<K> DeltaReplicatedData for PNCounterMap<K>
where
  K: Ord + Clone,
{
  type Delta = Self;

  fn delta(&self) -> Option<Self::Delta> {
    if self.delta.is_empty() && self.delta_removed_dots.is_empty() && self.delta_removed_values.is_empty() {
      None
    } else {
      Some(Self {
        entries:              self.delta.clone(),
        dots:                 self.delta_dots.clone(),
        removed_dots:         self.delta_removed_dots.clone(),
        removed_values:       self.delta_removed_values.clone(),
        delta:                BTreeMap::new(),
        delta_dots:           BTreeMap::new(),
        delta_removed_dots:   BTreeMap::new(),
        delta_removed_values: BTreeMap::new(),
      })
    }
  }

  fn merge_delta(&self, delta: &Self::Delta) -> Self {
    let mut entries = self.entries.clone();
    let mut dots = self.dots.clone();
    let mut removed_dots = self.removed_dots.clone();
    let mut removed_values = self.removed_values.clone();
    let mut local_delta = self.delta.clone();
    let mut local_delta_dots = self.delta_dots.clone();

    for (key, removed_key_dots) in &delta.removed_dots {
      merge_dot_map_entry(&mut removed_dots, key.clone(), removed_key_dots);
      if let Some(removed_value) = delta.removed_values.get(key) {
        merge_counter_map_entry(&mut removed_values, key.clone(), removed_value.clone());
      }
      apply_removed_dots(
        &mut entries,
        &mut dots,
        key,
        removed_key_dots,
        self.removed_dots.get(key),
        removed_values.get(key),
      );
      apply_removed_dots(
        &mut local_delta,
        &mut local_delta_dots,
        key,
        removed_key_dots,
        self.removed_dots.get(key),
        removed_values.get(key),
      );
    }

    for (key, counter) in &delta.entries {
      let incoming_dots = delta.dots.get(key).cloned().unwrap_or_default();
      let Some((counter, visible_dots)) = visible_counter(
        counter,
        &incoming_dots,
        removed_dots.get(key),
        delta.removed_dots.get(key),
        removed_values.get(key),
      ) else {
        continue;
      };
      let counter = counter.reset_delta();

      dots
        .entry(key.clone())
        .and_modify(|current| merge_dots_into(current, &visible_dots))
        .or_insert_with(|| visible_dots.clone());

      entries.entry(key.clone()).and_modify(|current| *current = current.merge(&counter)).or_insert(counter);
    }

    Self {
      entries,
      dots,
      removed_dots,
      removed_values,
      delta: local_delta,
      delta_dots: local_delta_dots,
      delta_removed_dots: self.delta_removed_dots.clone(),
      delta_removed_values: self.delta_removed_values.clone(),
    }
  }

  fn reset_delta(&self) -> Self {
    let entries = self.entries.iter().map(|(key, counter)| (key.clone(), counter.reset_delta())).collect();

    Self {
      entries,
      dots: self.dots.clone(),
      removed_dots: self.removed_dots.clone(),
      removed_values: self.removed_values.clone(),
      delta: BTreeMap::new(),
      delta_dots: BTreeMap::new(),
      delta_removed_dots: BTreeMap::new(),
      delta_removed_values: BTreeMap::new(),
    }
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

impl<K> RequiresCausalDeliveryOfDeltas for PNCounterMap<K> where K: Ord + Clone {}

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
    for key_dots in self.dots.values() {
      nodes.extend(key_dots.keys().cloned());
    }
    for key_dots in self.removed_dots.values() {
      nodes.extend(key_dots.keys().cloned());
    }
    for counter in self.removed_values.values() {
      nodes.extend(counter.modified_by_nodes());
    }
    nodes
  }

  fn need_pruning_from(&self, removed_node: &UniqueAddress) -> bool {
    self.entries.values().any(|counter| counter.need_pruning_from(removed_node))
      || self.dots.values().any(|key_dots| key_dots.contains_key(removed_node))
      || self.removed_dots.values().any(|key_dots| key_dots.contains_key(removed_node))
      || self.removed_values.values().any(|counter| counter.need_pruning_from(removed_node))
  }

  fn prune(&self, removed_node: &UniqueAddress, collapse_into: &UniqueAddress) -> Result<Self, Self::PruneError> {
    let mut entries = BTreeMap::new();
    let mut dots = BTreeMap::new();
    let mut removed_dots = BTreeMap::new();
    let mut removed_values = BTreeMap::new();
    let mut delta = BTreeMap::new();
    let mut delta_dots = BTreeMap::new();
    let delta_removed_dots: BTreeMap<K, BTreeMap<UniqueAddress, u64>> = self
      .delta_removed_dots
      .iter()
      .filter_map(|(key, key_dots)| {
        let key_dots = prune_tombstone_dots(key_dots, removed_node, collapse_into);
        if key_dots.is_empty() { None } else { Some((key.clone(), key_dots)) }
      })
      .collect();
    let mut delta_removed_values = BTreeMap::new();
    for (key, counter) in &self.delta_removed_values {
      let counter = counter.prune(removed_node, collapse_into)?;
      if !counter.modified_by_nodes().is_empty() {
        delta_removed_values.insert(key.clone(), counter.reset_delta());
      }
    }

    for (key, counter) in &self.entries {
      let counter = counter.prune(removed_node, collapse_into)?;
      let pruned_dots = self
        .dots
        .get(key)
        .map(|key_dots| prune_entry_dots(key_dots, removed_node, collapse_into, self.removed_dots.get(key)));
      let dots_changed = pruned_dots.as_ref() != self.dots.get(key);
      if counter.delta().is_some() || dots_changed {
        delta.insert(key.clone(), counter.reset_delta());
        if let Some(pruned_dots) = &pruned_dots {
          delta_dots.insert(key.clone(), pruned_dots.clone());
        }
      }
      entries.insert(key.clone(), counter);
    }

    for (key, key_dots) in &self.dots {
      dots.insert(key.clone(), prune_entry_dots(key_dots, removed_node, collapse_into, self.removed_dots.get(key)));
    }

    for (key, key_dots) in &self.removed_dots {
      let pruned_dots = prune_tombstone_dots(key_dots, removed_node, collapse_into);
      if !pruned_dots.is_empty() {
        removed_dots.insert(key.clone(), pruned_dots);
      }
    }

    for (key, counter) in &self.removed_values {
      let counter = counter.prune(removed_node, collapse_into)?;
      if !counter.modified_by_nodes().is_empty() {
        removed_values.insert(key.clone(), counter.reset_delta());
      }
    }

    Ok(Self {
      entries,
      dots,
      removed_dots,
      removed_values,
      delta,
      delta_dots,
      delta_removed_dots,
      delta_removed_values,
    })
  }

  fn pruning_cleanup(&self, removed_node: &UniqueAddress) -> Self {
    let mut entries = BTreeMap::new();
    let mut dots = BTreeMap::new();
    for (key, counter) in &self.entries {
      let key_dots = self.dots.get(key).map(|dots| pruning_cleanup_dots(dots, removed_node)).unwrap_or_default();
      if !key_dots.is_empty() {
        entries.insert(key.clone(), counter.pruning_cleanup(removed_node));
        dots.insert(key.clone(), key_dots);
      }
    }

    let removed_dots = self
      .removed_dots
      .iter()
      .filter_map(|(key, key_dots)| {
        let key_dots = pruning_cleanup_dots(key_dots, removed_node);
        if key_dots.is_empty() { None } else { Some((key.clone(), key_dots)) }
      })
      .collect();
    let removed_values = self
      .removed_values
      .iter()
      .filter_map(|(key, counter)| {
        let counter = counter.pruning_cleanup(removed_node);
        if counter.modified_by_nodes().is_empty() { None } else { Some((key.clone(), counter)) }
      })
      .collect();
    let delta = self
      .delta
      .iter()
      .filter_map(|(key, counter)| {
        let counter = counter.pruning_cleanup(removed_node);
        if counter.modified_by_nodes().is_empty() { None } else { Some((key.clone(), counter)) }
      })
      .collect();
    let delta_dots = self
      .delta_dots
      .iter()
      .filter_map(|(key, key_dots)| {
        let key_dots = pruning_cleanup_dots(key_dots, removed_node);
        if key_dots.is_empty() { None } else { Some((key.clone(), key_dots)) }
      })
      .collect();
    let delta_removed_dots = self
      .delta_removed_dots
      .iter()
      .filter_map(|(key, key_dots)| {
        let key_dots = pruning_cleanup_dots(key_dots, removed_node);
        if key_dots.is_empty() { None } else { Some((key.clone(), key_dots)) }
      })
      .collect();
    let delta_removed_values = self
      .delta_removed_values
      .iter()
      .filter_map(|(key, counter)| {
        let counter = counter.pruning_cleanup(removed_node);
        if counter.modified_by_nodes().is_empty() { None } else { Some((key.clone(), counter)) }
      })
      .collect();

    Self { entries, dots, removed_dots, removed_values, delta, delta_dots, delta_removed_dots, delta_removed_values }
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
      && self.dots == other.dots
      && self.removed_dots == other.removed_dots
      && self.removed_values == other.removed_values
  }
}

impl<K> Eq for PNCounterMap<K> where K: Ord {}

struct MergeEntrySide<'a, K> {
  entries:                 &'a BTreeMap<K, PNCounter>,
  dots:                    &'a BTreeMap<K, BTreeMap<UniqueAddress, u64>>,
  own_removed:             &'a BTreeMap<K, BTreeMap<UniqueAddress, u64>>,
  removed_by_other:        &'a BTreeMap<K, BTreeMap<UniqueAddress, u64>>,
  removed_values_by_other: &'a BTreeMap<K, PNCounter>,
}

fn merge_entries<K>(
  left: &MergeEntrySide<'_, K>,
  right: &MergeEntrySide<'_, K>,
) -> (BTreeMap<K, PNCounter>, BTreeMap<K, BTreeMap<UniqueAddress, u64>>)
where
  K: Ord + Clone, {
  debug_assert!(left.entries.keys().all(|key| left.dots.contains_key(key)));
  debug_assert!(right.entries.keys().all(|key| right.dots.contains_key(key)));

  let mut keys = BTreeSet::new();
  keys.extend(left.dots.keys().cloned());
  keys.extend(right.dots.keys().cloned());

  let mut entries = BTreeMap::new();
  let mut dots = BTreeMap::new();

  for key in keys {
    let left_visible = left.entries.get(&key).and_then(|counter| {
      visible_counter(
        counter,
        left.dots.get(&key)?,
        left.removed_by_other.get(&key),
        left.own_removed.get(&key),
        left.removed_values_by_other.get(&key),
      )
    });
    let right_visible = right.entries.get(&key).and_then(|counter| {
      visible_counter(
        counter,
        right.dots.get(&key)?,
        right.removed_by_other.get(&key),
        right.own_removed.get(&key),
        right.removed_values_by_other.get(&key),
      )
    });
    let left_visible_dots = left_visible.as_ref().map(|(_, dots)| dots.clone()).unwrap_or_default();
    let right_visible_dots = right_visible.as_ref().map(|(_, dots)| dots.clone()).unwrap_or_default();
    let left_value = left_visible.map(|(counter, _)| counter);
    let right_value = right_visible.map(|(counter, _)| counter);
    let value = match (left_value, right_value) {
      | (Some(left), Some(right)) => Some(left.merge(&right)),
      | (Some(left), None) => Some(left),
      | (None, Some(right)) => Some(right.reset_delta()),
      | (None, None) => None,
    };

    if let Some(value) = value {
      let merged_dots = merge_dots(&left_visible_dots, &right_visible_dots);
      if !merged_dots.is_empty() {
        dots.insert(key.clone(), merged_dots);
        entries.insert(key, value);
      }
    }
  }

  (entries, dots)
}

fn visible_counter(
  counter: &PNCounter,
  dots: &BTreeMap<UniqueAddress, u64>,
  removed_dots: Option<&BTreeMap<UniqueAddress, u64>>,
  own_removed_dots: Option<&BTreeMap<UniqueAddress, u64>>,
  removed_value: Option<&PNCounter>,
) -> Option<(PNCounter, BTreeMap<UniqueAddress, u64>)> {
  counter.retain_visible_nodes(dots, removed_dots, own_removed_dots, removed_value)
}

fn merge_dot_maps<K>(
  left: &BTreeMap<K, BTreeMap<UniqueAddress, u64>>,
  right: &BTreeMap<K, BTreeMap<UniqueAddress, u64>>,
) -> BTreeMap<K, BTreeMap<UniqueAddress, u64>>
where
  K: Ord + Clone, {
  let mut merged = left.clone();
  for (key, dots) in right {
    merge_dot_map_entry(&mut merged, key.clone(), dots);
  }
  merged
}

fn merge_counter_maps<K>(left: &BTreeMap<K, PNCounter>, right: &BTreeMap<K, PNCounter>) -> BTreeMap<K, PNCounter>
where
  K: Ord + Clone, {
  let mut merged = left.clone();
  for (key, counter) in right {
    merge_counter_map_entry(&mut merged, key.clone(), counter.clone());
  }
  merged
}

fn merge_counter_map_entry<K>(target: &mut BTreeMap<K, PNCounter>, key: K, incoming: PNCounter)
where
  K: Ord, {
  target.entry(key).and_modify(|current| *current = current.merge(&incoming)).or_insert(incoming);
}

fn merge_dot_map_entry<K>(
  target: &mut BTreeMap<K, BTreeMap<UniqueAddress, u64>>,
  key: K,
  incoming: &BTreeMap<UniqueAddress, u64>,
) where
  K: Ord, {
  target.entry(key).and_modify(|current| merge_dots_into(current, incoming)).or_insert_with(|| incoming.clone());
}

fn merge_dots(
  left: &BTreeMap<UniqueAddress, u64>,
  right: &BTreeMap<UniqueAddress, u64>,
) -> BTreeMap<UniqueAddress, u64> {
  let mut merged = left.clone();
  merge_dots_into(&mut merged, right);
  merged
}

fn merge_dots_into(target: &mut BTreeMap<UniqueAddress, u64>, incoming: &BTreeMap<UniqueAddress, u64>) {
  for (node, version) in incoming {
    target.entry(node.clone()).and_modify(|current| *current = (*current).max(*version)).or_insert(*version);
  }
}

fn apply_removed_dots<K>(
  entries: &mut BTreeMap<K, PNCounter>,
  dots: &mut BTreeMap<K, BTreeMap<UniqueAddress, u64>>,
  key: &K,
  removed_key_dots: &BTreeMap<UniqueAddress, u64>,
  own_removed_dots: Option<&BTreeMap<UniqueAddress, u64>>,
  removed_value: Option<&PNCounter>,
) where
  K: Ord + Clone, {
  let Some(current_dots) = dots.get(key).cloned() else {
    return;
  };

  if let Some(counter) = entries.get(key).cloned() {
    if let Some((counter, visible_dots)) =
      visible_counter(&counter, &current_dots, Some(removed_key_dots), own_removed_dots, removed_value)
    {
      entries.insert(key.clone(), counter);
      dots.insert(key.clone(), visible_dots);
    } else {
      dots.remove(key);
      entries.remove(key);
    }
  }
}

fn prune_entry_dots(
  key_dots: &BTreeMap<UniqueAddress, u64>,
  removed_node: &UniqueAddress,
  collapse_into: &UniqueAddress,
  removed_key_dots: Option<&BTreeMap<UniqueAddress, u64>>,
) -> BTreeMap<UniqueAddress, u64> {
  let mut pruned = key_dots.clone();
  if pruned.remove(removed_node).is_none() {
    return pruned;
  }

  if removed_node != collapse_into {
    let current = pruned.get(collapse_into).copied().unwrap_or(0);
    let removed = removed_key_dots.and_then(|key_dots| key_dots.get(collapse_into)).copied().unwrap_or(0);
    pruned.insert(collapse_into.clone(), current.max(removed).saturating_add(1));
  }

  pruned
}

fn prune_tombstone_dots(
  key_dots: &BTreeMap<UniqueAddress, u64>,
  removed_node: &UniqueAddress,
  collapse_into: &UniqueAddress,
) -> BTreeMap<UniqueAddress, u64> {
  let mut pruned = key_dots.clone();
  let Some(removed_version) = pruned.remove(removed_node) else {
    return pruned;
  };

  if removed_node != collapse_into {
    pruned
      .entry(collapse_into.clone())
      .and_modify(|current| *current = (*current).max(removed_version))
      .or_insert(removed_version);
  }

  pruned
}

fn pruning_cleanup_dots(
  key_dots: &BTreeMap<UniqueAddress, u64>,
  removed_node: &UniqueAddress,
) -> BTreeMap<UniqueAddress, u64> {
  let mut cleaned = key_dots.clone();
  cleaned.remove(removed_node);
  cleaned
}
