//! Observed-remove multi-map CRDT specialised over [`ORSet`] values.

#[cfg(test)]
#[path = "or_multimap_test.rs"]
mod tests;

use alloc::collections::{BTreeMap, BTreeSet};
use core::convert::Infallible;

use fraktor_remote_core_rs::address::UniqueAddress;

use super::{
  DeltaReplicatedData, ORMap, ORSet, RemovedNodePruning, ReplicatedData, ReplicatedDelta,
  RequiresCausalDeliveryOfDeltas, SelfUniqueAddress,
};

/// Observed-remove multi-map CRDT that associates a key with an observed-remove set of values.
///
/// Each key maps to an [`ORSet`] of elements. Adding a binding adds an element to the key's set;
/// removing the last binding removes the key from the visible entries. Concurrent add and remove of
/// the same binding follow the set's add-wins rule. Equality and the delta are delegated to the
/// underlying [`ORMap`].
#[derive(Debug, Clone)]
pub struct ORMultiMap<A, B> {
  underlying: ORMap<A, ORSet<B>>,
}

impl<A, B> ORMultiMap<A, B>
where
  A: Clone + Ord,
  B: Clone + Ord,
{
  /// Creates an empty multi-map.
  #[must_use]
  pub const fn new() -> Self {
    Self { underlying: ORMap::new() }
  }

  /// Returns a multi-map with `element` added to the set bound to `key`.
  #[must_use]
  pub fn add_binding(&self, node: &SelfUniqueAddress, key: A, element: B) -> Self {
    Self { underlying: self.underlying.update(node, key, ORSet::new(), |set| set.add(node, element)) }
  }

  /// Returns a multi-map with `element` removed from the set bound to `key`.
  ///
  /// When the set becomes empty the key is removed from the visible entries.
  #[must_use]
  pub fn remove_binding(&self, node: &SelfUniqueAddress, key: &A, element: &B) -> Self {
    let Some(existing) = self.underlying.get(key) else {
      return self.clone();
    };
    if !existing.contains(element) {
      return self.clone();
    }

    let updated = self.underlying.update(node, key.clone(), ORSet::new(), |set| set.remove(element));
    Self { underlying: updated }
  }

  /// Returns the set of values bound to `key`, or `None` when absent.
  #[must_use]
  pub fn get(&self, key: &A) -> Option<BTreeSet<B>> {
    self.underlying.get(key).filter(|set| !set.is_empty()).map(ORSet::elements)
  }

  /// Returns the visible entries with their value sets.
  #[must_use]
  pub fn entries(&self) -> BTreeMap<A, BTreeSet<B>> {
    self
      .underlying
      .entries()
      .iter()
      .filter(|(_, set)| !set.is_empty())
      .map(|(key, set)| (key.clone(), set.elements()))
      .collect()
  }

  /// Returns true when `key` has a visible value set.
  #[must_use]
  pub fn contains_key(&self, key: &A) -> bool {
    self.get(key).is_some()
  }

  /// Returns true when the multi-map has no visible entries.
  #[must_use]
  pub fn is_empty(&self) -> bool {
    self.underlying.entries().values().all(ORSet::is_empty)
  }

  /// Returns the number of visible keys.
  #[must_use]
  pub fn len(&self) -> usize {
    self.underlying.entries().values().filter(|set| !set.is_empty()).count()
  }
}

impl<A, B> ReplicatedData for ORMultiMap<A, B>
where
  A: Clone + Ord,
  B: Clone + Ord,
{
  fn merge(&self, other: &Self) -> Self {
    Self { underlying: self.underlying.merge(&other.underlying) }
  }
}

impl<A, B> DeltaReplicatedData for ORMultiMap<A, B>
where
  A: Clone + Ord,
  B: Clone + Ord,
{
  type Delta = Self;

  fn delta(&self) -> Option<Self::Delta> {
    self.underlying.delta().map(|underlying| Self { underlying })
  }

  fn merge_delta(&self, delta: &Self::Delta) -> Self {
    Self { underlying: self.underlying.merge_delta(&delta.underlying) }
  }

  fn reset_delta(&self) -> Self {
    Self { underlying: self.underlying.reset_delta() }
  }
}

impl<A, B> ReplicatedDelta for ORMultiMap<A, B>
where
  A: Clone + Ord,
  B: Clone + Ord,
{
  type Full = Self;

  fn zero(&self) -> Self::Full {
    let _ = self;
    Self::new()
  }
}

impl<A, B> RequiresCausalDeliveryOfDeltas for ORMultiMap<A, B>
where
  A: Clone + Ord,
  B: Clone + Ord,
{
}

impl<A, B> RemovedNodePruning for ORMultiMap<A, B>
where
  A: Clone + Ord,
  B: Clone + Ord,
{
  type PruneError = Infallible;

  fn modified_by_nodes(&self) -> BTreeSet<UniqueAddress> {
    self.underlying.modified_by_nodes()
  }

  fn need_pruning_from(&self, removed_node: &UniqueAddress) -> bool {
    self.underlying.need_pruning_from(removed_node)
  }

  fn prune(&self, removed_node: &UniqueAddress, collapse_into: &UniqueAddress) -> Result<Self, Self::PruneError> {
    match self.underlying.prune(removed_node, collapse_into) {
      | Ok(underlying) => Ok(Self { underlying }),
      | Err(never) => match never {},
    }
  }

  fn pruning_cleanup(&self, removed_node: &UniqueAddress) -> Self {
    Self { underlying: self.underlying.pruning_cleanup(removed_node) }
  }
}

impl<A, B> Default for ORMultiMap<A, B>
where
  A: Clone + Ord,
  B: Clone + Ord,
{
  fn default() -> Self {
    Self::new()
  }
}

impl<A, B> PartialEq for ORMultiMap<A, B>
where
  A: Ord,
  B: Ord,
{
  fn eq(&self, other: &Self) -> bool {
    self.underlying == other.underlying
  }
}

impl<A, B> Eq for ORMultiMap<A, B>
where
  A: Ord,
  B: Ord,
{
}
