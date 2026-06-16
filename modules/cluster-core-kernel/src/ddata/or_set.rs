//! Observed-remove set CRDT with add-wins semantics.

#[cfg(test)]
#[path = "or_set_test.rs"]
mod tests;

use alloc::{
  collections::{BTreeMap, BTreeSet},
  vec::Vec,
};
use core::convert::Infallible;

use fraktor_remote_core_rs::address::UniqueAddress;

use super::{
  DeltaReplicatedData, RemovedNodePruning, ReplicatedData, ReplicatedDelta, RequiresCausalDeliveryOfDeltas,
  SelfUniqueAddress, VersionVector,
};

/// Observed-remove set CRDT, also known as ORSWOT, where a concurrent add wins over a remove.
///
/// Each element stores a birth dot keyed by the adding node. A remove drops the element without a
/// tombstone; the set version vector records the observed dots so a concurrent add on another
/// replica survives the next merge. Equality ignores the local delta marker because it does not
/// affect future merges. The delta is the accumulated full state since the last reset.
#[derive(Debug, Clone)]
pub struct ORSet<A> {
  elements:    BTreeMap<A, VersionVector>,
  vvector:     VersionVector,
  delta_dirty: bool,
}

impl<A> ORSet<A>
where
  A: Clone + Ord,
{
  /// Creates an empty set.
  #[must_use]
  pub const fn new() -> Self {
    Self { elements: BTreeMap::new(), vvector: VersionVector::new(), delta_dirty: false }
  }

  /// Returns a set with `element` added under the local node identity.
  #[must_use]
  pub fn add(&self, node: &SelfUniqueAddress, element: A) -> Self {
    self.add_at(node.unique_address(), element)
  }

  fn add_at(&self, node: &UniqueAddress, element: A) -> Self {
    let next = self.vvector.version_at(node).saturating_add(1);
    let dot = single_dot(node, next);
    let vvector = self.vvector.merge(&dot);
    let mut elements = self.elements.clone();
    let dots = elements.get(&element).map_or_else(|| dot.clone(), |current| current.merge(&dot));
    elements.insert(element, dots);
    Self { elements, vvector, delta_dirty: true }
  }

  /// Returns a set with the observed `element` removed.
  ///
  /// A concurrent add on another replica that this replica has not observed survives a later merge.
  #[must_use]
  pub fn remove(&self, element: &A) -> Self {
    if !self.elements.contains_key(element) {
      return self.clone();
    }

    let mut elements = self.elements.clone();
    elements.remove(element);
    Self { elements, vvector: self.vvector.clone(), delta_dirty: true }
  }

  /// Returns an empty set that keeps the causal history.
  #[must_use]
  pub fn clear(&self) -> Self {
    Self { elements: BTreeMap::new(), vvector: self.vvector.clone(), delta_dirty: true }
  }

  /// Returns true when `element` is visible.
  #[must_use]
  pub fn contains(&self, element: &A) -> bool {
    self.elements.contains_key(element)
  }

  /// Returns the visible elements in deterministic order.
  #[must_use]
  pub fn elements(&self) -> BTreeSet<A> {
    self.elements.keys().cloned().collect()
  }

  /// Returns true when the set has no visible elements.
  #[must_use]
  pub fn is_empty(&self) -> bool {
    self.elements.is_empty()
  }

  /// Returns the number of visible elements.
  #[must_use]
  pub fn len(&self) -> usize {
    self.elements.len()
  }

  pub(super) fn dots_for(&self, element: &A) -> Option<&VersionVector> {
    self.elements.get(element)
  }
}

impl<A> ReplicatedData for ORSet<A>
where
  A: Clone + Ord,
{
  fn merge(&self, other: &Self) -> Self {
    let mut elements = BTreeMap::new();

    for (element, lhs_dots) in &self.elements {
      let Some(rhs_dots) = other.elements.get(element) else {
        continue;
      };
      let common = common_dots(lhs_dots, rhs_dots);
      let lhs_keep = unique_dots(lhs_dots, rhs_dots).subtract_dots(&other.vvector);
      let rhs_keep = unique_dots(rhs_dots, lhs_dots).subtract_dots(&self.vvector);
      let merged = lhs_keep.merge(&rhs_keep).merge(&common);
      if !merged.is_empty() {
        elements.insert(element.clone(), merged);
      }
    }

    for (element, lhs_dots) in &self.elements {
      if other.elements.contains_key(element) {
        continue;
      }
      let kept = lhs_dots.subtract_dots(&other.vvector);
      if !kept.is_empty() {
        elements.insert(element.clone(), kept);
      }
    }

    for (element, rhs_dots) in &other.elements {
      if self.elements.contains_key(element) {
        continue;
      }
      let kept = rhs_dots.subtract_dots(&self.vvector);
      if !kept.is_empty() {
        elements.insert(element.clone(), kept);
      }
    }

    let vvector = self.vvector.merge(&other.vvector);
    Self { elements, vvector, delta_dirty: self.delta_dirty }
  }
}

impl<A> DeltaReplicatedData for ORSet<A>
where
  A: Clone + Ord,
{
  type Delta = Self;

  fn delta(&self) -> Option<Self::Delta> {
    if self.delta_dirty {
      Some(Self { elements: self.elements.clone(), vvector: self.vvector.clone(), delta_dirty: false })
    } else {
      None
    }
  }

  fn merge_delta(&self, delta: &Self::Delta) -> Self {
    self.merge(delta)
  }

  fn reset_delta(&self) -> Self {
    Self { elements: self.elements.clone(), vvector: self.vvector.clone(), delta_dirty: false }
  }
}

impl<A> ReplicatedDelta for ORSet<A>
where
  A: Clone + Ord,
{
  type Full = Self;

  fn zero(&self) -> Self::Full {
    let _ = self;
    Self::new()
  }
}

impl<A> RequiresCausalDeliveryOfDeltas for ORSet<A> where A: Clone + Ord {}

impl<A> RemovedNodePruning for ORSet<A>
where
  A: Clone + Ord,
{
  type PruneError = Infallible;

  fn modified_by_nodes(&self) -> BTreeSet<UniqueAddress> {
    self.vvector.modified_by_nodes()
  }

  fn need_pruning_from(&self, removed_node: &UniqueAddress) -> bool {
    self.vvector.need_pruning_from(removed_node)
  }

  fn prune(&self, removed_node: &UniqueAddress, collapse_into: &UniqueAddress) -> Result<Self, Self::PruneError> {
    if removed_node == collapse_into {
      return Ok(self.clone());
    }

    let mut pruned: BTreeMap<A, VersionVector> = BTreeMap::new();
    for (element, dots) in &self.elements {
      if dots.contains_node(removed_node) {
        pruned.insert(element.clone(), cleanup_version_vector(dots, removed_node));
      }
    }

    let vvector = cleanup_version_vector(&self.vvector, removed_node);

    if pruned.is_empty() {
      let delta_dirty = self.delta_dirty || vvector != self.vvector;
      return Ok(Self { elements: self.elements.clone(), vvector, delta_dirty });
    }

    let mut elements = self.elements.clone();
    for (element, dots) in &pruned {
      elements.insert(element.clone(), dots.clone());
    }

    let mut vvector = vvector;
    for element in pruned.keys() {
      let next = vvector.version_at(collapse_into).saturating_add(1);
      let collapse_dot = single_dot(collapse_into, next);
      vvector = vvector.merge(&collapse_dot);
      if let Some(current) = elements.get(element).cloned() {
        elements.insert(element.clone(), current.merge(&collapse_dot));
      }
    }
    Ok(Self { elements, vvector, delta_dirty: true })
  }

  fn pruning_cleanup(&self, removed_node: &UniqueAddress) -> Self {
    let mut elements = BTreeMap::new();
    for (element, dots) in &self.elements {
      let cleaned = cleanup_version_vector(dots, removed_node);
      if !cleaned.is_empty() {
        elements.insert(element.clone(), cleaned);
      }
    }

    let vvector = cleanup_version_vector(&self.vvector, removed_node);
    let delta_dirty = self.delta_dirty || elements != self.elements || vvector != self.vvector;
    Self { elements, vvector, delta_dirty }
  }
}

impl<A> Default for ORSet<A>
where
  A: Clone + Ord,
{
  fn default() -> Self {
    Self::new()
  }
}

impl<A> PartialEq for ORSet<A>
where
  A: Ord,
{
  fn eq(&self, other: &Self) -> bool {
    self.elements == other.elements && self.vvector == other.vvector
  }
}

impl<A> Eq for ORSet<A> where A: Ord {}

fn single_dot(node: &UniqueAddress, version: u64) -> VersionVector {
  VersionVector::from_entries([(node.clone(), version)])
}

fn common_dots(lhs: &VersionVector, rhs: &VersionVector) -> VersionVector {
  let mut entries: Vec<(UniqueAddress, u64)> = Vec::new();
  for (node, version) in lhs.entries() {
    if rhs.version_at(node) == version {
      entries.push((node.clone(), version));
    }
  }
  VersionVector::from_entries(entries)
}

fn unique_dots(dots: &VersionVector, other: &VersionVector) -> VersionVector {
  let mut entries: Vec<(UniqueAddress, u64)> = Vec::new();
  for (node, version) in dots.entries() {
    if other.version_at(node) != version {
      entries.push((node.clone(), version));
    }
  }
  VersionVector::from_entries(entries)
}

fn cleanup_version_vector(vvector: &VersionVector, removed_node: &UniqueAddress) -> VersionVector {
  let mut entries: Vec<(UniqueAddress, u64)> = Vec::new();
  for (node, version) in vvector.entries() {
    if node != removed_node {
      entries.push((node.clone(), version));
    }
  }
  VersionVector::from_entries(entries)
}
