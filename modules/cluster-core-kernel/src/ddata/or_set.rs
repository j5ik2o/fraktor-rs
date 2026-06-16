//! Observed-remove set CRDT with add-wins semantics.

#[cfg(test)]
#[path = "or_set_test.rs"]
mod tests;

use alloc::{
  collections::{BTreeMap, BTreeSet},
  format,
  vec::Vec,
};
use core::convert::Infallible;

use fraktor_remote_core_rs::address::{Address, UniqueAddress};

use super::{
  DeltaReplicatedData, RemovedNodePruning, ReplicatedData, ReplicatedDelta, RequiresCausalDeliveryOfDeltas,
  SelfUniqueAddress, VersionVector,
};

/// Observed-remove set CRDT, also known as ORSWOT, where a concurrent add wins over a remove.
///
/// Each element stores a birth dot keyed by the adding node. Removed element dots are retained as
/// tombstone history so a stale observed add cannot be resurrected by a later merge. Equality
/// ignores the local delta marker because it does not affect future merges. The delta is the
/// accumulated full state since the last reset.
#[derive(Debug, Clone)]
pub struct ORSet<A> {
  elements:     BTreeMap<A, VersionVector>,
  removed_dots: BTreeMap<A, VersionVector>,
  vvector:      VersionVector,
  delta_dirty:  bool,
}

impl<A> ORSet<A>
where
  A: Clone + Ord,
{
  /// Creates an empty set.
  #[must_use]
  pub const fn new() -> Self {
    Self {
      elements:     BTreeMap::new(),
      removed_dots: BTreeMap::new(),
      vvector:      VersionVector::new(),
      delta_dirty:  false,
    }
  }

  /// Returns a set with `element` added under the local node identity.
  #[must_use]
  pub fn add(&self, node: &SelfUniqueAddress, element: A) -> Self {
    self.add_at(node.unique_address(), element)
  }

  fn add_at(&self, node: &UniqueAddress, element: A) -> Self {
    let next = next_dot_version(&self.vvector, node);
    let dot = single_dot(node, next);
    let vvector = self.vvector.merge(&dot);
    let mut elements = self.elements.clone();
    let dots = elements.get(&element).map_or_else(|| dot.clone(), |current| current.merge(&dot));
    elements.insert(element, dots);
    Self { elements, removed_dots: self.removed_dots.clone(), vvector, delta_dirty: true }
  }

  /// Returns a set with the observed `element` removed.
  ///
  /// A concurrent add on another replica that this replica has not observed survives a later merge.
  #[must_use]
  pub fn remove(&self, element: &A) -> Self {
    let Some(observed_dots) = self.elements.get(element).cloned() else {
      return self.clone();
    };

    let mut elements = self.elements.clone();
    elements.remove(element);

    let mut removed_dots = self.removed_dots.clone();
    merge_removed_dot_entry(&mut removed_dots, element.clone(), &observed_dots);

    Self { elements, removed_dots, vvector: self.vvector.clone(), delta_dirty: true }
  }

  /// Returns an empty set that keeps the causal history.
  #[must_use]
  pub fn clear(&self) -> Self {
    let mut removed_dots = self.removed_dots.clone();
    for (element, dots) in &self.elements {
      merge_removed_dot_entry(&mut removed_dots, element.clone(), dots);
    }

    Self { elements: BTreeMap::new(), removed_dots, vvector: self.vvector.clone(), delta_dirty: true }
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

  fn observed_dots_for(&self, element: &A) -> VersionVector {
    self.removed_dots.get(element).map_or_else(|| self.vvector.clone(), |removed_dots| self.vvector.merge(removed_dots))
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
      let lhs_keep = subtract_observed_dots(&unique_dots(lhs_dots, rhs_dots), &other.observed_dots_for(element));
      let rhs_keep = subtract_observed_dots(&unique_dots(rhs_dots, lhs_dots), &self.observed_dots_for(element));
      let merged = lhs_keep.merge(&rhs_keep).merge(&common);
      if !merged.is_empty() {
        elements.insert(element.clone(), merged);
      }
    }

    for (element, lhs_dots) in &self.elements {
      if other.elements.contains_key(element) {
        continue;
      }
      let kept = subtract_observed_dots(lhs_dots, &other.observed_dots_for(element));
      if !kept.is_empty() {
        elements.insert(element.clone(), kept);
      }
    }

    for (element, rhs_dots) in &other.elements {
      if self.elements.contains_key(element) {
        continue;
      }
      let kept = subtract_observed_dots(rhs_dots, &self.observed_dots_for(element));
      if !kept.is_empty() {
        elements.insert(element.clone(), kept);
      }
    }

    let vvector = self.vvector.merge(&other.vvector);
    let removed_dots = merge_removed_dot_maps(&self.removed_dots, &other.removed_dots);
    Self { elements, removed_dots, vvector, delta_dirty: self.delta_dirty }
  }
}

impl<A> DeltaReplicatedData for ORSet<A>
where
  A: Clone + Ord,
{
  type Delta = Self;

  fn delta(&self) -> Option<Self::Delta> {
    if self.delta_dirty {
      Some(Self {
        elements:     self.elements.clone(),
        removed_dots: self.removed_dots.clone(),
        vvector:      self.vvector.clone(),
        delta_dirty:  false,
      })
    } else {
      None
    }
  }

  fn merge_delta(&self, delta: &Self::Delta) -> Self {
    self.merge(delta)
  }

  fn reset_delta(&self) -> Self {
    Self {
      elements:     self.elements.clone(),
      removed_dots: self.removed_dots.clone(),
      vvector:      self.vvector.clone(),
      delta_dirty:  false,
    }
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
    self.vvector.need_pruning_from(removed_node) || self.elements.values().any(|dots| dots.contains_node(removed_node))
  }

  fn prune(&self, removed_node: &UniqueAddress, collapse_into: &UniqueAddress) -> Result<Self, Self::PruneError> {
    if removed_node == collapse_into {
      return Ok(self.clone());
    }

    let pruning_node = pruning_node_for(removed_node);
    let mut elements = BTreeMap::new();
    let mut vvector = cleanup_version_vector(&self.vvector, removed_node);
    for (element, dots) in &self.elements {
      let pruned = prune_element_dots(dots, removed_node, &pruning_node);
      vvector = vvector.merge(&pruned);
      elements.insert(element.clone(), pruned);
    }

    let mut removed_dots = BTreeMap::new();
    for (element, dots) in &self.removed_dots {
      let pruned = prune_removed_dots(dots, removed_node, &pruning_node);
      vvector = vvector.merge(&cleanup_version_vector(&pruned, removed_node));
      removed_dots.insert(element.clone(), pruned);
    }

    let delta_dirty =
      self.delta_dirty || elements != self.elements || removed_dots != self.removed_dots || vvector != self.vvector;
    Ok(Self { elements, removed_dots, vvector, delta_dirty })
  }

  fn pruning_cleanup(&self, removed_node: &UniqueAddress) -> Self {
    let mut elements = BTreeMap::new();
    for (element, dots) in &self.elements {
      let cleaned = cleanup_version_vector(dots, removed_node);
      if !cleaned.is_empty() {
        elements.insert(element.clone(), cleaned);
      }
    }

    let mut removed_dots = BTreeMap::new();
    for (element, dots) in &self.removed_dots {
      let cleaned = cleanup_version_vector(dots, removed_node);
      if !cleaned.is_empty() {
        removed_dots.insert(element.clone(), cleaned);
      }
    }

    let vvector = cleanup_version_vector(&self.vvector, removed_node);
    let delta_dirty =
      self.delta_dirty || elements != self.elements || removed_dots != self.removed_dots || vvector != self.vvector;
    Self { elements, removed_dots, vvector, delta_dirty }
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
    self.elements == other.elements && self.removed_dots == other.removed_dots && self.vvector == other.vvector
  }
}

impl<A> Eq for ORSet<A> where A: Ord {}

fn single_dot(node: &UniqueAddress, version: u64) -> VersionVector {
  VersionVector::from_entries([(node.clone(), version)])
}

fn next_dot_version(vvector: &VersionVector, node: &UniqueAddress) -> u64 {
  match vvector.version_at(node).checked_add(1) {
    | Some(next) => next,
    | None => panic!("ORSet dot version overflow"),
  }
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

fn subtract_observed_dots(dots: &VersionVector, observed: &VersionVector) -> VersionVector {
  let mut entries: Vec<(UniqueAddress, u64)> = Vec::new();
  for (node, version) in dots.entries() {
    if !observes_dot(observed, node, version) {
      entries.push((node.clone(), version));
    }
  }
  VersionVector::from_entries(entries)
}

fn observes_dot(observed: &VersionVector, node: &UniqueAddress, version: u64) -> bool {
  if observed.version_at(node) >= version {
    return true;
  }

  let pruned_node = pruning_node_for(node);
  observed.entries().any(|(observed_node, observed_version)| {
    observed_version >= version && (*observed_node == pruned_node || pruning_node_for(observed_node) == *node)
  })
}

fn merge_removed_dot_entry<A>(removed_dots: &mut BTreeMap<A, VersionVector>, element: A, dots: &VersionVector)
where
  A: Ord, {
  removed_dots.entry(element).and_modify(|current| *current = current.merge(dots)).or_insert_with(|| dots.clone());
}

fn merge_removed_dot_maps<A>(
  left: &BTreeMap<A, VersionVector>,
  right: &BTreeMap<A, VersionVector>,
) -> BTreeMap<A, VersionVector>
where
  A: Clone + Ord, {
  let mut merged = left.clone();
  for (element, dots) in right {
    merge_removed_dot_entry(&mut merged, element.clone(), dots);
  }
  merged
}

fn pruning_node_for(removed_node: &UniqueAddress) -> UniqueAddress {
  let address = removed_node.address();
  UniqueAddress::new(
    Address::new(address.system(), format!("{}#pruned-{}", address.host(), removed_node.uid()), address.port()),
    0,
  )
}

fn prune_element_dots(
  dots: &VersionVector,
  removed_node: &UniqueAddress,
  pruning_node: &UniqueAddress,
) -> VersionVector {
  let removed_version = dots.version_at(removed_node);
  if removed_version == 0 {
    return dots.clone();
  }

  let mut entries: Vec<(UniqueAddress, u64)> = Vec::new();
  for (node, version) in dots.entries() {
    if node != removed_node {
      entries.push((node.clone(), version));
    }
  }
  entries.push((pruning_node.clone(), dots.version_at(pruning_node).max(removed_version)));
  VersionVector::from_entries(entries)
}

fn prune_removed_dots(
  dots: &VersionVector,
  removed_node: &UniqueAddress,
  pruning_node: &UniqueAddress,
) -> VersionVector {
  let removed_version = dots.version_at(removed_node);
  if removed_version == 0 {
    return dots.clone();
  }

  dots.merge(&single_dot(pruning_node, dots.version_at(pruning_node).max(removed_version)))
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
