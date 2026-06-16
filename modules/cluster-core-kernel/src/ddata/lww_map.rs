//! Last-writer-wins map CRDT specialised over [`LWWRegister`] values.

#[cfg(test)]
#[path = "lww_map_test.rs"]
mod tests;

use alloc::collections::{BTreeMap, BTreeSet};
use core::convert::Infallible;

use fraktor_remote_core_rs::address::UniqueAddress;

use super::{
  DeltaReplicatedData, LWWRegister, ORMap, RemovedNodePruning, ReplicatedData, ReplicatedDelta,
  RequiresCausalDeliveryOfDeltas, SelfUniqueAddress,
};

/// Observed-remove map CRDT whose per-key values follow last-writer-wins semantics.
///
/// Keys are tracked as in [`ORMap`]; each value is an [`LWWRegister`]. A concurrent put on the same
/// key keeps the value with the greatest timestamp, breaking ties by the lowest `UniqueAddress`.
/// Equality and the delta are delegated to the underlying [`ORMap`].
#[derive(Debug, Clone)]
pub struct LWWMap<A, B> {
  underlying: ORMap<A, LWWRegister<B>>,
}

impl<A, B> LWWMap<A, B>
where
  A: Clone + Ord,
  B: Clone,
{
  /// Creates an empty map.
  #[must_use]
  pub const fn new() -> Self {
    Self { underlying: ORMap::new() }
  }

  /// Returns a map with `value` stored at `key` using the default last-writer-wins clock.
  #[must_use]
  pub fn put(&self, node: &SelfUniqueAddress, key: A, value: B, current_time_millis: i64) -> Self {
    self
      .put_with_clock(node, key, value, |timestamp, _| LWWRegister::<B>::default_clock(timestamp, current_time_millis))
  }

  /// Returns a map with `value` stored at `key` whose timestamp is selected by `clock`.
  #[must_use]
  pub fn put_with_clock(&self, node: &SelfUniqueAddress, key: A, value: B, clock: impl FnOnce(i64, &B) -> i64) -> Self {
    let register = match self.underlying.get(&key) {
      | Some(existing) => existing.with_value_with_clock(node, value, clock).unwrap_or_else(|| existing.clone()),
      | None => LWWRegister::new_with_clock(node, value, clock),
    };
    Self { underlying: self.underlying.put(node, key, register) }
  }

  /// Returns a map with the observed entry for `key` removed.
  #[must_use]
  pub fn remove(&self, key: &A) -> Self {
    Self { underlying: self.underlying.remove(key) }
  }

  /// Returns the value associated with `key`, or `None` when absent.
  #[must_use]
  pub fn get(&self, key: &A) -> Option<&B> {
    self.underlying.get(key).map(LWWRegister::value)
  }

  /// Returns the visible entries with their current values.
  #[must_use]
  pub fn entries(&self) -> BTreeMap<A, B> {
    self.underlying.entries().iter().map(|(key, register)| (key.clone(), register.value().clone())).collect()
  }

  /// Returns true when `key` has a visible value.
  #[must_use]
  pub fn contains_key(&self, key: &A) -> bool {
    self.underlying.contains_key(key)
  }

  /// Returns true when the map has no visible entries.
  #[must_use]
  pub fn is_empty(&self) -> bool {
    self.underlying.is_empty()
  }

  /// Returns the number of visible entries.
  #[must_use]
  pub fn len(&self) -> usize {
    self.underlying.len()
  }
}

impl<A, B> ReplicatedData for LWWMap<A, B>
where
  A: Clone + Ord,
  B: Clone,
{
  fn merge(&self, other: &Self) -> Self {
    Self { underlying: self.underlying.merge(&other.underlying) }
  }
}

impl<A, B> DeltaReplicatedData for LWWMap<A, B>
where
  A: Clone + Ord,
  B: Clone,
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

impl<A, B> ReplicatedDelta for LWWMap<A, B>
where
  A: Clone + Ord,
  B: Clone,
{
  type Full = Self;

  fn zero(&self) -> Self::Full {
    let _ = self;
    Self::new()
  }
}

impl<A, B> RequiresCausalDeliveryOfDeltas for LWWMap<A, B>
where
  A: Clone + Ord,
  B: Clone,
{
}

impl<A, B> RemovedNodePruning for LWWMap<A, B>
where
  A: Clone + Ord,
  B: Clone,
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

impl<A, B> Default for LWWMap<A, B>
where
  A: Clone + Ord,
  B: Clone,
{
  fn default() -> Self {
    Self::new()
  }
}

impl<A, B> PartialEq for LWWMap<A, B>
where
  A: Ord,
  B: PartialEq,
{
  fn eq(&self, other: &Self) -> bool {
    self.underlying == other.underlying
  }
}

impl<A, B> Eq for LWWMap<A, B>
where
  A: Ord,
  B: Eq,
{
}
