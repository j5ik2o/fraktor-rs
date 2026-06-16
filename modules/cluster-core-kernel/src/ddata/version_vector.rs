//! Version vector CRDT for causal history tracking.

#[cfg(test)]
#[path = "version_vector_test.rs"]
mod tests;

use alloc::collections::{BTreeMap, BTreeSet};

use fraktor_remote_core_rs::address::UniqueAddress;

use super::{CounterArithmeticError, RemovedNodePruning, ReplicatedData, SelfUniqueAddress, VersionVectorOrdering};

/// Version vector CRDT keyed by unique cluster node identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionVector {
  versions: BTreeMap<UniqueAddress, u64>,
}

impl VersionVector {
  /// Creates an empty version vector.
  #[must_use]
  pub const fn new() -> Self {
    Self { versions: BTreeMap::new() }
  }

  /// Creates a version vector from node-version entries.
  ///
  /// Duplicate node entries keep the highest version. Zero versions are omitted because absence is
  /// the canonical zero value.
  #[must_use]
  pub fn from_entries(entries: impl IntoIterator<Item = (UniqueAddress, u64)>) -> Self {
    let mut versions = BTreeMap::new();
    for (node, version) in entries {
      if version == 0 {
        continue;
      }
      let current = versions.get(&node).copied().unwrap_or(0);
      if version > current {
        versions.insert(node, version);
      }
    }
    Self { versions }
  }

  /// Returns true when the vector has no node versions.
  #[must_use]
  pub fn is_empty(&self) -> bool {
    self.versions.is_empty()
  }

  /// Returns the number of node versions.
  #[must_use]
  pub fn len(&self) -> usize {
    self.versions.len()
  }

  /// Returns true when the vector contains a version for `node`.
  #[must_use]
  pub fn contains_node(&self, node: &UniqueAddress) -> bool {
    self.versions.contains_key(node)
  }

  /// Returns the version for `node`, or zero when the node is absent.
  #[must_use]
  pub fn version_at(&self, node: &UniqueAddress) -> u64 {
    self.versions.get(node).copied().unwrap_or(0)
  }

  /// Returns all node-version entries in deterministic node order.
  pub fn entries(&self) -> impl Iterator<Item = (&UniqueAddress, u64)> {
    self.versions.iter().map(|(node, version)| (node, *version))
  }

  /// Returns a vector with the local node version incremented.
  ///
  /// # Errors
  ///
  /// Returns [`CounterArithmeticError::Overflow`] when the node version cannot be incremented.
  pub fn increment(&self, node: &SelfUniqueAddress) -> Result<Self, CounterArithmeticError> {
    self.increment_unique_address(node.unique_address())
  }

  /// Compares this vector with `other` using vector-clock causal ordering.
  #[must_use]
  pub fn compare(&self, other: &Self) -> VersionVectorOrdering {
    let mut has_less = false;
    let mut has_greater = false;

    for (node, left_version) in &self.versions {
      let right_version = other.version_at(node);
      update_ordering_flags(*left_version, right_version, &mut has_less, &mut has_greater);
      if has_less && has_greater {
        return VersionVectorOrdering::Concurrent;
      }
    }

    for (node, right_version) in &other.versions {
      if self.versions.contains_key(node) {
        continue;
      }
      update_ordering_flags(0, *right_version, &mut has_less, &mut has_greater);
      if has_less && has_greater {
        return VersionVectorOrdering::Concurrent;
      }
    }

    match (has_less, has_greater) {
      | (false, false) => VersionVectorOrdering::Same,
      | (true, false) => VersionVectorOrdering::Before,
      | (false, true) => VersionVectorOrdering::After,
      | (true, true) => VersionVectorOrdering::Concurrent,
    }
  }

  /// Returns true when this vector happened before `other`.
  #[must_use]
  pub fn is_before(&self, other: &Self) -> bool {
    self.compare(other) == VersionVectorOrdering::Before
  }

  /// Returns true when this vector happened after `other`.
  #[must_use]
  pub fn is_after(&self, other: &Self) -> bool {
    self.compare(other) == VersionVectorOrdering::After
  }

  /// Returns true when this vector contains the same history as `other`.
  #[must_use]
  pub fn is_same(&self, other: &Self) -> bool {
    self.compare(other) == VersionVectorOrdering::Same
  }

  /// Returns true when this vector and `other` contain independent histories.
  #[must_use]
  pub fn is_concurrent(&self, other: &Self) -> bool {
    self.compare(other) == VersionVectorOrdering::Concurrent
  }

  /// Returns the dots in this vector that are not observed by `vvector`.
  ///
  /// An entry `(node, version)` is retained when `vvector` has not observed it, that is when
  /// `vvector.version_at(node) < version`. Observed-remove collections use this to keep concurrent
  /// additions while merging.
  #[must_use]
  pub fn subtract_dots(&self, vvector: &Self) -> Self {
    let mut versions = BTreeMap::new();
    for (node, version) in &self.versions {
      if vvector.version_at(node) < *version {
        versions.insert(node.clone(), *version);
      }
    }
    Self { versions }
  }

  fn increment_unique_address(&self, node: &UniqueAddress) -> Result<Self, CounterArithmeticError> {
    let next = self.version_at(node).checked_add(1).ok_or(CounterArithmeticError::Overflow)?;
    let mut versions = self.versions.clone();
    versions.insert(node.clone(), next);
    Ok(Self { versions })
  }
}

impl ReplicatedData for VersionVector {
  fn merge(&self, other: &Self) -> Self {
    let mut versions = self.versions.clone();
    for (node, version) in &other.versions {
      let current = versions.get(node).copied().unwrap_or(0);
      if *version > current {
        versions.insert(node.clone(), *version);
      }
    }
    Self { versions }
  }
}

impl RemovedNodePruning for VersionVector {
  type PruneError = CounterArithmeticError;

  fn modified_by_nodes(&self) -> BTreeSet<UniqueAddress> {
    self.versions.keys().cloned().collect()
  }

  fn need_pruning_from(&self, removed_node: &UniqueAddress) -> bool {
    self.versions.contains_key(removed_node)
  }

  fn prune(&self, removed_node: &UniqueAddress, collapse_into: &UniqueAddress) -> Result<Self, Self::PruneError> {
    let removed_version = self.version_at(removed_node);
    if removed_version == 0 {
      return Ok(self.clone());
    }

    let mut versions = self.versions.clone();
    versions.remove(removed_node);

    if removed_node != collapse_into {
      let collapse_version = versions.get(collapse_into).copied().unwrap_or(0);
      let next = collapse_version.checked_add(1).ok_or(CounterArithmeticError::Overflow)?;
      versions.insert(collapse_into.clone(), next);
    }

    Ok(Self { versions })
  }

  fn pruning_cleanup(&self, removed_node: &UniqueAddress) -> Self {
    let mut versions = self.versions.clone();
    versions.remove(removed_node);
    Self { versions }
  }
}

impl Default for VersionVector {
  fn default() -> Self {
    Self::new()
  }
}

const fn update_ordering_flags(left: u64, right: u64, has_less: &mut bool, has_greater: &mut bool) {
  if left < right {
    *has_less = true;
  } else if left > right {
    *has_greater = true;
  }
}
