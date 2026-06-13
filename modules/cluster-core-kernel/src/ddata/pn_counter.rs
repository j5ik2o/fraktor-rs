//! Positive-negative counter CRDT.

#[cfg(test)]
#[path = "pn_counter_test.rs"]
mod tests;

use alloc::collections::BTreeSet;
use core::convert::TryFrom;

use fraktor_remote_core_rs::address::UniqueAddress;

use super::{
  CounterArithmeticError, DeltaReplicatedData, GCounter, RemovedNodePruning, ReplicatedData, ReplicatedDelta,
  SelfUniqueAddress,
};

const I128_MIN_ABS: u128 = 1_u128 << 127;

/// Counter CRDT supporting both increments and decrements.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PNCounter {
  increments: GCounter,
  decrements: GCounter,
}

impl PNCounter {
  /// Creates an empty positive-negative counter.
  #[must_use]
  pub const fn new() -> Self {
    Self::from_parts(GCounter::new(), GCounter::new())
  }

  pub(super) const fn from_parts(increments: GCounter, decrements: GCounter) -> Self {
    Self { increments, decrements }
  }

  /// Returns a counter with `n` added to the positive component.
  ///
  /// # Errors
  ///
  /// Returns [`CounterArithmeticError::Overflow`] when the positive component cannot represent the
  /// new value.
  pub fn increment(&self, node: &SelfUniqueAddress, n: u64) -> Result<Self, CounterArithmeticError> {
    Ok(Self { increments: self.increments.increment(node, n)?, decrements: self.decrements.clone() })
  }

  /// Returns a counter with `n` added to the negative component.
  ///
  /// # Errors
  ///
  /// Returns [`CounterArithmeticError::Overflow`] when the negative component cannot represent the
  /// new value.
  pub fn decrement(&self, node: &SelfUniqueAddress, n: u64) -> Result<Self, CounterArithmeticError> {
    Ok(Self { increments: self.increments.clone(), decrements: self.decrements.increment(node, n)? })
  }

  /// Returns the signed value of this counter.
  ///
  /// # Errors
  ///
  /// Returns [`CounterArithmeticError::Overflow`] when either component sum or the signed
  /// difference is out of range.
  pub fn value(&self) -> Result<i128, CounterArithmeticError> {
    signed_difference(self.increments.value()?, self.decrements.value()?)
  }
}

impl ReplicatedData for PNCounter {
  fn merge(&self, other: &Self) -> Self {
    Self { increments: self.increments.merge(&other.increments), decrements: self.decrements.merge(&other.decrements) }
  }
}

impl DeltaReplicatedData for PNCounter {
  type Delta = Self;

  fn delta(&self) -> Option<Self::Delta> {
    let increments = self.increments.delta();
    let decrements = self.decrements.delta();

    if increments.is_none() && decrements.is_none() {
      None
    } else {
      Some(Self { increments: increments.unwrap_or_default(), decrements: decrements.unwrap_or_default() })
    }
  }

  fn merge_delta(&self, delta: &Self::Delta) -> Self {
    self.merge(delta)
  }

  fn reset_delta(&self) -> Self {
    Self { increments: self.increments.reset_delta(), decrements: self.decrements.reset_delta() }
  }
}

impl ReplicatedDelta for PNCounter {
  type Full = Self;

  #[allow(clippy::unused_self)] // The trait models delta payloads; zero depends only on the full-state type.
  fn zero(&self) -> Self::Full {
    Self::new()
  }
}

impl RemovedNodePruning for PNCounter {
  type PruneError = CounterArithmeticError;

  fn modified_by_nodes(&self) -> BTreeSet<UniqueAddress> {
    let mut nodes = self.increments.modified_by_nodes();
    nodes.extend(self.decrements.modified_by_nodes());
    nodes
  }

  fn need_pruning_from(&self, removed_node: &UniqueAddress) -> bool {
    self.increments.need_pruning_from(removed_node) || self.decrements.need_pruning_from(removed_node)
  }

  fn prune(&self, removed_node: &UniqueAddress, collapse_into: &UniqueAddress) -> Result<Self, Self::PruneError> {
    Ok(Self {
      increments: self.increments.prune(removed_node, collapse_into)?,
      decrements: self.decrements.prune(removed_node, collapse_into)?,
    })
  }

  fn pruning_cleanup(&self, removed_node: &UniqueAddress) -> Self {
    Self {
      increments: self.increments.pruning_cleanup(removed_node),
      decrements: self.decrements.pruning_cleanup(removed_node),
    }
  }
}

impl Default for PNCounter {
  fn default() -> Self {
    Self::new()
  }
}

fn signed_difference(increments: u128, decrements: u128) -> Result<i128, CounterArithmeticError> {
  if increments >= decrements {
    i128::try_from(increments - decrements).map_err(|_| CounterArithmeticError::Overflow)
  } else {
    let diff = decrements - increments;
    if diff == I128_MIN_ABS {
      Ok(i128::MIN)
    } else {
      let magnitude = i128::try_from(diff).map_err(|_| CounterArithmeticError::Overflow)?;
      Ok(-magnitude)
    }
  }
}
